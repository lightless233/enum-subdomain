use std::{collections::HashMap, process::exit, sync::Arc};

use async_channel::Sender;
use async_trait::async_trait;
use itertools::Itertools;
use tokio::{fs::read_to_string, sync::Mutex};

use crate::{
    args::AppArgs,
    context::{AppContext, EngineStatus},
};

#[async_trait]
trait TaskBuilderTrait {
    async fn build(task_channel: Sender<String>, app_args: &Arc<AppArgs>);
}

/// 通过枚举进行构建任务
struct EnumBuilder {}
#[async_trait]
impl TaskBuilderTrait for EnumBuilder {
    async fn build(task_channel: Sender<String>, app_args: &Arc<AppArgs>) {
        println!("TaskBuilder EnumBuilder start!");
        // 字符池
        let mut pool = ('a'..='z').chain('0'..='9').collect::<Vec<_>>();
        pool.push('-');

        let length = app_args.length;
        let range = length.0..=length.1;
        for idx in range {
            println!("start build lenght {idx} word.");
            let product = (1..=idx).map(|_| pool.iter()).multi_cartesian_product();
            for it in product {
                let task = it.iter().cloned().join("");
                if task.starts_with('-') {
                    continue;
                }

                if let Err(e) = task_channel.send(task.clone()).await {
                    eprintln!("Error put task to channel, task:{}, error:{:?}", task, e)
                }
            }
        }

        println!("TaskBuilder EnumBuilder finished!");
    }
}

/// 通过字典构建任务
struct DictBuilder {}
impl DictBuilder {
    /// 根据参数决定使用内置字典还是从文件读字典
    async fn get_dict_content(dict_path: Option<&String>) -> Result<String, std::io::Error> {
        let dict_path = dict_path.unwrap();
        if dict_path.is_empty() {
            // 使用内置字典
            println!("No dict specified, use default dict.");
            Ok(include_str!("../../dicts/default.txt").to_owned())
        } else {
            // 读取用户提供的字典文件
            let content = read_to_string(dict_path).await?;
            Ok(content)
        }
    }

    /// 解析字典的每一行，按照占位符拆开
    fn extract_line(line: &str) -> Vec<String> {
        let mut fsm_status: u8 = 0;
        let mut result: Vec<String> = vec![];
        let mut tmp_buffer: Vec<char> = vec![];

        for c in line.chars() {
            if c == '%' {
                match fsm_status {
                    0 => {
                        // 开始进入 pat，把 tmp_buffer 清空，开始记录 pat
                        if !tmp_buffer.is_empty() {
                            result.push(tmp_buffer.iter().collect::<String>());
                            tmp_buffer.clear();
                        }
                        tmp_buffer.push(c);
                        fsm_status = 1;
                    }
                    1 => {
                        // pat 结束的标志
                        tmp_buffer.push(c);
                        result.push(tmp_buffer.iter().collect::<String>());
                        tmp_buffer.clear();
                        fsm_status = 0;
                    }
                    _ => continue,
                }
            } else {
                tmp_buffer.push(c);
            }
        }
        if !tmp_buffer.is_empty() {
            result.push(tmp_buffer.iter().collect::<String>());
        }

        result
    }
}

#[async_trait]
impl TaskBuilderTrait for DictBuilder {
    async fn build(task_channel: Sender<String>, app_args: &Arc<AppArgs>) {
        println!("TaskBuilder DictBuilder start!");

        // 读取字典内容
        let dict_path = app_args.dict_path.as_ref();
        let content = DictBuilder::get_dict_content(dict_path).await;
        if let Err(e) = content {
            eprintln!("Read dict failed, path: {:?}, error: {:?}", dict_path, e);
            exit(-1);
        }

        // 为 pattern 构建 pool
        let mut pools = HashMap::new();
        pools.insert("%NUMBER%", ('0'..='9').collect_vec());
        pools.insert("%ALPHA%", ('a'..='z').collect_vec());
        pools.insert("%ALPHANUMBER%", ('0'..='9').chain('a'..='z').collect_vec());
        for (_, v) in pools.iter_mut() {
            v.push('-');
        }

        // 逐步展开每一行字典
        for line in content.unwrap().lines() {
            // skip empty line and comment line
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // 如果某一项是以 . 结尾的，去掉末尾的 .
            let item = if line.ends_with('.') {
                line.trim_end_matches('.')
            } else {
                line
            };

            // 展开每一行，一行有可能展开出多个任务
            let mut tasks: Vec<String> = vec![];
            let line_parts = DictBuilder::extract_line(item);
            for pat in line_parts {
                match pools.get(pat.as_str()) {
                    Some(pool) => {
                        // 当前部分是占位符
                        if tasks.is_empty() {
                            pool.iter().for_each(|it| tasks.push(it.to_string()));
                        } else {
                            let tmp = tasks.clone();
                            let product = tmp.iter().cartesian_product(pool);
                            tasks.clear();
                            product.for_each(|it| tasks.push(format!("{}{}", it.0, it.1)));
                        }
                    }
                    None => {
                        // 当前不是占位符
                        if tasks.is_empty() {
                            tasks.push(pat);
                        } else {
                            let tmp = tasks.clone();
                            tasks.clear();
                            for item in tmp {
                                tasks.push(format!("{}{}", item, pat));
                            }
                        }
                    }
                }
            }

            for task in tasks {
                if let Err(e) = task_channel.send(task.clone()).await {
                    eprintln!(
                        "Error put task to channel, line: {}, task: {}, error: {:?}",
                        line, task, e
                    );
                }
            }
        }

        println!("TaskBuilder DictBuilder finished!");
    }
}

/// task builder engine
pub async fn task_builder(
    task_channel: Sender<String>,
    app_args: Arc<AppArgs>,
    app_context: Arc<Mutex<AppContext>>,
) {
    let mut guard = app_context.lock().await;
    guard.task_builder_status = EngineStatus::Running;
    drop(guard);

    if app_args.dict_path.is_some() {
        DictBuilder::build(task_channel, &app_args).await;
    } else {
        EnumBuilder::build(task_channel, &app_args).await;
    }

    app_context.lock().await.task_builder_status = EngineStatus::Stop;
}
