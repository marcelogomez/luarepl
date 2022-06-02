use rlua::Error;
use rlua::Lua;
use rlua::Value;
use std::io::BufRead;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;

#[derive(Debug)]
struct Session {
    expr_sender: Sender<String>,
    eval_thread: JoinHandle<()>,
}

impl Session {
    pub fn new() -> Self {
        let (expr_sender, expr_receiver) = std::sync::mpsc::channel::<String>();
        let eval_thread = thread::spawn(move || {
            let lua = Lua::new();
            lua.context(|ctx| {
                for expr in expr_receiver.into_iter() {
                    println!("{:?}", ctx.load(&expr).eval::<Value>());
                }
            });
        });

        Self {
            expr_sender,
            eval_thread,
        }
    }

    pub fn eval(&mut self, expr: String) {
        self.expr_sender.send(expr);
    }
}

fn main() {
    let mut session = Session::new();
    for line in std::io::stdin().lock().lines() {
        session.eval(line.unwrap());
    }
    session.eval_thread.join();
}
