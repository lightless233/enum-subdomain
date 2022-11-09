use args::AppArgs;

mod args;
mod context;
mod engines;

#[tokio::main]
async fn main() {
    let mut app_args = AppArgs::default();
    app_args.parse_cli_arguments();

    // 初始化消息队列
    // 任务通道
    let (task_tx, task_rx) = async_channel::bounded::<String>(10240);
    let (saver_tx, saver_rx) = async_channel::bounded::<String>(1024);

    println!("app_args: {:?}", app_args);
}
