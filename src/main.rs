use std::{process::exit, sync::Arc};

use args::AppArgs;
use resolver::{build_resolver, check_wildcard};
use tokio::sync::Mutex;

use crate::{
    context::{AppContext, ResolveResult},
    engines::{resolver, saver, task_builder},
};

mod args;
mod context;
mod engines;

#[tokio::main]
async fn main() {
    let mut app_args = AppArgs::default();
    app_args.parse_cli_arguments();
    println!("app_args: {:?}", app_args);

    // 初始化消息队列
    // 任务通道
    let (task_tx, task_rx) = async_channel::bounded::<String>(10240);
    let (saver_tx, saver_rx) = async_channel::bounded::<ResolveResult>(1024);

    let app_args = Arc::new(app_args);
    let app_context = Arc::new(Mutex::new(AppContext::new()));

    // 先进行一次泛解析检查，如果有泛解析直接退出，不要等到后面再检查泛解析
    if app_args.check_wildcard {
        let resolver = build_resolver(&app_args.nameserver_list).unwrap();
        if let Err(e) = check_wildcard(&app_args.target, &resolver).await {
            eprintln!("Find wildcard record, IP list: {:?}", e);
            exit(-1);
        } else {
            println!("No wildcard records.")
        }
    }

    // 启动 task_builder
    let task_builder = tokio::spawn(task_builder(
        task_tx.clone(),
        app_args.clone(),
        app_context.clone(),
    ));

    // 启动 resolver
    let mut resolvers = vec![];
    for idx in 0..app_args.task_count {
        let mut guard = app_context.lock().await;
        guard.resolver_status.push(context::EngineStatus::Init);
        drop(guard);
        let h = tokio::spawn(resolver(
            task_rx.clone(),
            saver_tx.clone(),
            idx,
            app_args.clone(),
            app_context.clone(),
        ));
        resolvers.push(h);
    }

    // 启动 saver
    let saver = tokio::spawn(saver(
        saver_rx.clone(),
        app_context.clone(),
        app_args.clone(),
    ));

    // 等待所有任务结束
    let _ = task_builder.await;
    for h in resolvers {
        let _ = h.await;
    }
    let _ = saver.await;
}
