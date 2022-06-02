use rlua::Context;
use rlua::Error;
use rlua::Function;
use rlua::Lua;
use rlua::Table;
use rlua::Value;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::BufRead;
use std::thread;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

#[derive(Debug, PartialEq)]
struct EvalResponse {
    success: bool,
    objects: HashMap<String, LuaObject>,
    value: LuaValue,
}

#[derive(Debug, PartialEq)]
enum LuaValue {
    Nil,
    Boolean(bool),
    Number(f64),
    String(String),
    ObjectRef(String),
}

#[derive(Debug, PartialEq)]
struct LuaObject {
    members: Vec<(LuaValue, LuaValue)>,
}

impl LuaObject {
    pub fn new() -> Self {
        Self { members: vec![] }
    }

    pub fn insert(&mut self, key: LuaValue, value: LuaValue) {
        self.members.push((key, value));
    }
}

fn parse_value<'l>(
    ctx: Context<'l>,
    rlua_value: Value<'l>,
    objects: &mut HashMap<String, LuaObject>,
    seen_objs: &mut HashSet<String>,
) -> LuaValue {
    match rlua_value {
        Value::Table(t) => {
            let to_string: Function = ctx.globals().get("tostring").unwrap();
            parse_table(ctx, t.clone(), objects, seen_objs);
            LuaValue::ObjectRef(to_string.call::<_, String>(t).unwrap())
        }
        Value::Boolean(b) => LuaValue::Boolean(b),
        Value::String(s) => LuaValue::String(s.to_str().unwrap_or_default().to_string()),
        Value::Number(n) => LuaValue::Number(n),
        Value::Integer(n) => LuaValue::Number(n as f64),
        Value::Nil => LuaValue::Nil,
        v => panic!("Error: Not yet supported {:?}", v),
    }
}

fn parse_table<'lua>(
    ctx: Context<'lua>,
    table: Table<'lua>,
    objects: &mut HashMap<String, LuaObject>,
    seen_objs: &mut HashSet<String>,
) -> String {
    let to_string: Function = ctx.globals().get("tostring").unwrap();
    let table_id = to_string.call::<_, String>(table.clone()).unwrap();

    if seen_objs.insert(table_id.clone()) {
        let mut object = LuaObject::new();
        for (k, v) in table
            .pairs::<Value, Value>()
            .into_iter()
            .map(|r| r.unwrap())
        {
            object.insert(
                parse_value(ctx, k, objects, seen_objs),
                parse_value(ctx, v, objects, seen_objs),
            );
        }
        objects.insert(table_id.clone(), object);
    }

    table_id
}

impl EvalResponse {
    fn from_result<'l>(ctx: Context<'l>, eval_result: Result<Value<'l>, Error>) -> Self {
        match eval_result {
            Err(_e) => Self {
                success: false,
                objects: HashMap::new(),
                value: LuaValue::Nil,
            },
            Ok(v) => Self::from_value(ctx, v),
        }
    }

    fn from_value<'l>(ctx: Context<'l>, value: Value<'l>) -> Self {
        match value {
            Value::Boolean(b) => Self {
                success: true,
                objects: HashMap::new(),
                value: LuaValue::Boolean(b),
            },
            Value::String(s) => Self {
                success: true,
                objects: HashMap::new(),
                value: LuaValue::String(s.to_str().unwrap_or_default().to_string()),
            },
            Value::Number(n) => Self {
                success: true,
                objects: HashMap::new(),
                value: LuaValue::Number(n),
            },
            Value::Integer(n) => Self {
                success: true,
                objects: HashMap::new(),
                value: LuaValue::Number(n as f64),
            },
            Value::Nil => Self {
                success: true,
                objects: HashMap::new(),
                value: LuaValue::Nil,
            },
            Value::Table(t) => {
                let mut objects = HashMap::new();
                let mut seen_objs = HashSet::new();
                let table_id = parse_table(ctx, t, &mut objects, &mut seen_objs);
                Self {
                    success: true,
                    objects,
                    value: LuaValue::ObjectRef(table_id),
                }
            }
            v => panic!("Value not yet supported {:?}", v),
        }
    }
}

#[derive(Debug)]
struct Session {
    expr_sender: UnboundedSender<String>,
    result_receiver: UnboundedReceiver<EvalResponse>,
    eval_thread: JoinHandle<()>,
}

impl Session {
    pub fn new() -> Self {
        let (expr_sender, mut expr_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (result_sender, result_receiver) = tokio::sync::mpsc::unbounded_channel();
        let eval_thread = tokio::spawn(async move {
            let lua = Lua::new();
            let (inner_sender, inner_receiver) = std::sync::mpsc::channel::<String>();
            let eval_thread = thread::spawn(move || {
                lua.context(|ctx| {
                    inner_receiver
                        .into_iter()
                        .map(|expr| ctx.load(&expr).eval::<Value>())
                        .for_each(|result| {
                            // TODO: handle this
                            let _ = result_sender.send(EvalResponse::from_result(ctx, result));
                        });
                });
            });

            while let Some(expr) = expr_receiver.recv().await {
                let _ = inner_sender.send(expr);
            }
            let _ = eval_thread.join();
        });

        Self {
            result_receiver,
            expr_sender,
            eval_thread,
        }
    }

    pub async fn eval(&mut self, expr: String) -> EvalResponse {
        let _ = self.expr_sender.send(expr);
        self.result_receiver.recv().await.unwrap()
    }
}

#[tokio::main]
async fn main() {
    let mut session = Session::new();
    for line in std::io::stdin().lock().lines() {
        println!("{:#?}", session.eval(line.unwrap()).await);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_simple() {
        let mut session = Session::new();

        assert_eq!(
            session.eval("x = 1".to_string()).await,
            EvalResponse {
                success: true,
                objects: HashMap::new(),
                value: LuaValue::Nil,
            }
        );

        assert_eq!(
            session.eval("return x".to_string()).await,
            EvalResponse {
                success: true,
                objects: HashMap::new(),
                value: LuaValue::Number(1.0),
            }
        );
    }

    #[tokio::test]
    async fn test_syntax_error() {
        let mut session = Session::new();

        assert_eq!(
            session.eval("syntax error".to_string()).await,
            EvalResponse {
                success: false,
                objects: HashMap::new(),
                value: LuaValue::Nil,
            }
        );
    }

    #[tokio::test]
    async fn test_simple_table() {
        let mut session = Session::new();
        let resp = session.eval("x = {}; return x".to_string()).await;

        assert!(resp.success);
        let table_id = if let LuaValue::ObjectRef(id) = &resp.value {
            id.to_string()
        } else {
            panic!("Expected an object ref got {:?}!", resp.value);
        };

        let resp = session.eval("x['a'] = 1 ; return x".to_string()).await;
        assert!(resp.success);
        assert_eq!(
            resp.objects,
            vec![(
                table_id,
                LuaObject {
                    members: vec![(LuaValue::String("a".to_string()), LuaValue::Number(1.0))]
                }
            )].into_iter().collect(),
       );
    }
}
