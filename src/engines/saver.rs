use std::{sync::Arc, time::Duration};

use async_channel::Receiver;
use tokio::io::AsyncWriteExt;
use tokio::{fs::File, sync::Mutex};

use crate::{
    args::AppArgs,
    context::{AppContext, ResolveResult},
};

pub async fn saver(
    rx: Receiver<ResolveResult>,
    app_context: Arc<Mutex<AppContext>>,
    app_args: Arc<AppArgs>,
) {
    println!("saver engine start.");
    let output = &app_args.output_path;
    let mut output_file = File::create(output).await.unwrap();

    loop {
        let result = rx.try_recv();
        if result.is_err() {
            if !app_context
                .lock()
                .await
                .resolver_status
                .contains(&crate::context::EngineStatus::Running)
            {
                break;
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        }

        // 把结果格式化成行，临时先简单的格式化即可
        let result = result.unwrap();
        // println!("Found: {:?}", result);
        let line = format!(
            "{} - {:?} - {:?} - {:?} - {:?}\n",
            result.domain,
            result.ip,
            result.cname,
            result.code.unwrap_or(0),
            result.title.unwrap_or("".to_string()),
        );
        if let Err(e) = output_file.write(line.as_bytes()).await {
            eprintln!("write file error, value: {:?}, error: {:?}", line, e)
        };
    }

    println!("saver engine finished.");
}
