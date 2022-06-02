use rlua::Lua;
use rlua::Value;
use std::io::BufRead;
use std::thread;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

#[derive(Debug)]
struct Session {
    expr_sender: UnboundedSender<String>,
    eval_thread: JoinHandle<()>,
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
                        .for_each(|(result, expr)| println!("{} -> {:?}", expr, result));
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
