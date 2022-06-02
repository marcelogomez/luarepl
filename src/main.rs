use rlua::Lua;
use rlua::Value;
use rlua::Error;
use std::io::BufRead;

#[derive(Debug)]
struct Session {
    expressions: Vec<String>,
    lua_instance: Lua,
}

impl Session {
    pub fn new() -> Self {
        Self {
            expressions: vec![],
            lua_instance: Lua::new(),
        }
    }

    pub fn eval(&mut self, expr: String) {
        self.expressions.push(expr);
        self.lua_instance.context(|ctx| {
            let results = self.expressions
                .iter()
                .map(|exp| ctx.load(exp).eval::<Value>())
                .collect::<Vec<Result<Value, Error>>>();
            
            println!("{:#?}", results);
        });
    }
}

fn main() {
    let mut session = Session::new();
    for line in std::io::stdin().lock().lines() {
        session.eval(line.unwrap());
    }
}
