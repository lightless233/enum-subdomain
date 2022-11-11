use std::{net::SocketAddr, sync::Arc, time::Duration};

use async_channel::{Receiver, Sender};
use rand::{distributions::Alphanumeric, Rng};
use regex::Regex;
use reqwest::{Client, ClientBuilder};
use tokio::sync::Mutex;
use trust_dns_resolver::{
    config::{NameServerConfig, Protocol, ResolverConfig, ResolverOpts},
    error::ResolveError,
    name_server::{GenericConnection, GenericConnectionProvider, TokioRuntime},
    proto::rr::RecordType,
    AsyncResolver, TokioAsyncResolver,
};

use crate::{
    args::AppArgs,
    context::{AppContext, EngineStatus, ResolveResult},
};

pub async fn resolver(
    task_channel: Receiver<String>,
    result_channel: Sender<ResolveResult>,
    idx: usize,
    app_args: Arc<AppArgs>,
    app_context: Arc<Mutex<AppContext>>,
) {
    // println!("Resolver engine {idx} start!");
    let mut guard = app_context.lock().await;
    guard.resolver_status[idx] = EngineStatus::Running;
    drop(guard);

    // 目标
    let target = &app_args.target;

    // 构建 dns resolver
    let resolver = build_resolver(&app_args.nameserver_list).expect("Build DNS Resolver Error!");

    // 构建 http client
    let http_client = ClientBuilder::new()
        .timeout(Duration::from_secs(9))
        .build()
        .unwrap();

    // 获取网页标题的正则
    let title_regex = Regex::new(r"<title.*?>(?P<title>.+?)</title>").unwrap();

    loop {
        let task = task_channel.try_recv();
        if task.is_err() {
            if app_context.lock().await.task_builder_status == EngineStatus::Stop {
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
            continue;
        }

        // 开始解析域名
        let task = task.unwrap();
        let domain = format!("{}.{}", task, target);
        let (cname_list, ip_list) = dns_worker(domain.as_str(), &resolver).await;

        // 如果有结果，并且配置了获取 title 就发起请求
        let mut status_code: Option<u16> = None;
        let mut title: Option<String> = None;
        if !ip_list.is_empty() && app_args.fetch_title {
            (status_code, title) = http_worker(domain.as_str(), &title_regex, &http_client).await;
        }

        // 把解析结果扔到队列里
        if !cname_list.is_empty() || !ip_list.is_empty() {
            let res = ResolveResult {
                domain,
                title,
                code: status_code,
                ip: ip_list,
                cname: cname_list,
            };
            println!("Found: {:?}", res);
            if let Err(e) = result_channel.send(res).await {
                eprintln!("Error put task to result_channel, error: {:?}", e);
            }
        }
    }

    app_context.lock().await.resolver_status[idx] = EngineStatus::Stop;
    // println!("Resolver engine {idx} finished!");
}

/// 构建 DNS Resolver
pub fn build_resolver(
    nameservers: &Vec<String>,
) -> Result<AsyncResolver<GenericConnection, GenericConnectionProvider<TokioRuntime>>, ResolveError>
{
    let resolve_config = if nameservers.is_empty() {
        // 使用内置的 google DNS
        ResolverConfig::google()
    } else {
        // 使用用户提供的 NS IP
        let mut resolve_config = ResolverConfig::default();
        for ns_ip in nameservers {
            match format!("{}:53", ns_ip).parse::<SocketAddr>() {
                Ok(ip) => resolve_config.add_name_server(NameServerConfig::new(ip, Protocol::Udp)),
                Err(e) => {
                    eprintln!("Invalid Nameserver IP {}, error: {:?}, skip.", ns_ip, e);
                    continue;
                }
            };
        }
        resolve_config
    };

    TokioAsyncResolver::tokio(resolve_config, ResolverOpts::default())
}

/// 检查泛解析
pub async fn check_wildcard(
    target: &str,
    resolver: &AsyncResolver<GenericConnection, GenericConnectionProvider<TokioRuntime>>,
) -> Result<(), String> {
    println!("start checking wildcard resolve.");
    let mut wildcards: Vec<String> = vec!["thisdomainneverexist".into()];
    let rand_subdomain = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(5)
        .map(char::from)
        .collect();
    wildcards.push(rand_subdomain);
    println!("domain list: {:?}", wildcards);

    for wildcard in wildcards {
        let full_domain = format!("{}.{}", wildcard, target);
        if let Ok(resp) = resolver.lookup_ip(&full_domain).await {
            let ip_list = resp.iter().collect::<Vec<_>>();
            if !ip_list.is_empty() {
                return Err(format!("{:?}", ip_list));
            }
        }
    }
    Ok(())
}

/// 解析域名到 IP
async fn dns_worker(
    target: &str,
    resolver: &AsyncResolver<GenericConnection, GenericConnectionProvider<TokioRuntime>>,
) -> (Vec<String>, Vec<String>) {
    let mut cname_list: Vec<String> = vec![];
    let mut ip_list: Vec<String> = vec![];

    if let Ok(resp) = resolver.lookup(target, RecordType::CNAME).await {
        cname_list.extend(resp.iter().map(|it| it.to_string()));
    }

    if let Ok(resp) = resolver.lookup_ip(target).await {
        ip_list.extend(resp.iter().map(|it| it.to_string()));
    }

    (cname_list, ip_list)
}

/// 获取 HTTP 状态码和网页 title
async fn http_worker(
    target: &str,
    re: &Regex,
    http_client: &Client,
) -> (Option<u16>, Option<String>) {
    let url = format!("http://{}", target);
    match http_client.get(url).send().await {
        Ok(resp) => {
            let code = resp.status().as_u16();
            let html = resp.text().await.unwrap();
            let mut title: Option<String> = None;
            if let Some(caps) = re.captures(&html) {
                let _title = caps.name("title").map_or("", |m| m.as_str()).to_owned();
                title = Some(_title);
            }
            (Some(code), title)
        }
        Err(e) => {
            eprintln!("Fetch HTTP status_code and title error. error: {:?}", e);
            (None, None)
        }
    }
}
