use rlua::Error;
use rlua::Lua;
use rlua::Value;
use std::collections::HashMap;
use std::io::BufRead;
use std::thread;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

#[derive(Debug)]
struct Session {
    expr_sender: UnboundedSender<String>,
    eval_thread: JoinHandle<()>,
}

#[derive(Debug)]
struct EvalResponse {
    success: bool,
    objects: HashMap<String, LuaObject>,
    value: LuaValue,
}

#[derive(Debug)]
enum LuaValue {
    Nil,
    Boolean(bool),
    Number(f64),
    String(String),
    ObjectRef(String),
}

#[derive(Debug)]
struct LuaObject {
    members: Vec<(String, LuaValue)>,
}

impl From<Result<Value<'_>, Error>> for EvalResponse {
    fn from(eval_result: Result<Value<'_>, Error>) -> Self {
        match eval_result {
            Err(_e) => Self {
                success: false,
                objects: HashMap::new(),
                value: LuaValue::Nil,
            },
            Ok(v) => Self::from(v),
        }
    }
}

impl From<Value<'_>> for EvalResponse {
    fn from(value: Value) -> Self {
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
            v => panic!("Value not yet supported {:?}", v),
        }
    }
}

impl Session {
    pub fn new() -> Self {
        let (expr_sender, mut expr_receiver) = tokio::sync::mpsc::unbounded_channel();
        let eval_thread = tokio::spawn(async move {
            let lua = Lua::new();
            let (inner_sender, inner_receiver) = std::sync::mpsc::channel::<String>();
            let eval_thread = thread::spawn(move || {
                lua.context(|ctx| {
                    inner_receiver
                        .into_iter()
                        .map(|expr| (ctx.load(&expr).eval::<Value>(), expr))
                        .for_each(|(result, expr)| {
                            println!("{} -> {:?}", expr, EvalResponse::from(result))
                        });
                });
            });

            while let Some(expr) = expr_receiver.recv().await {
                let _ = inner_sender.send(expr);
            }
            let _ = eval_thread.join();
        });

        Self {
            expr_sender,
            eval_thread,
        }
    }

    pub async fn eval(&mut self, expr: String) {
        let _ = self.expr_sender.send(expr);
    }
}

#[tokio::main]
async fn main() {
    let mut session = Session::new();
    for line in std::io::stdin().lock().lines() {
        session.eval(line.unwrap()).await;
    }
}
