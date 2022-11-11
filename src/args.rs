use std::path::Path;

use clap::{command, value_parser, Arg, ArgAction, Command};

#[derive(Debug, Default)]
pub struct AppArgs {
    // 待枚举的域名
    pub target: String,

    // 字典模式，如果留空使用内置字典
    pub dict_path: Option<String>,

    // 爆破模式，与字典模式只能启用一个
    pub length: (usize, usize),

    // 输出文件，默认使用 "{target}.txt"
    pub output_path: String,

    // 任务数量
    pub task_count: usize,

    // 是否跳过泛解析检查
    pub check_wildcard: bool,

    // ns 服务器列表
    pub nameserver_list: Vec<String>,

    // 是否跳过爆破出域名的 title 获取
    pub fetch_title: bool,
}

impl AppArgs {
    fn build_command(&self) -> Command {
        command!()
            .arg_required_else_help(true)
            .arg(Arg::new("target").required(true).help("待爆破的域名"))
            .arg(
                Arg::new("dict")
                    .short('d')
                    .long("dict")
                    .num_args(0..=1)
                    .conflicts_with("length")
                    .default_missing_value("")
                    .help("字典路径，留空使用内置字典"),
            )
            .arg(
                Arg::new("length")
                    .short('l')
                    .long("length")
                    .conflicts_with("dict")
                    .help("爆破模式的长度"),
            )
            .arg(
                Arg::new("output")
                    .short('o')
                    .long("output")
                    .help("扫描结果的输出路径"),
            )
            .arg(
                Arg::new("task-count")
                    .short('c')
                    .long("task-count")
                    .default_value("25")
                    .value_parser(value_parser!(usize))
                    .help("扫描线程的数量，默认25"),
            )
            .arg(
                Arg::new("nameserver")
                    .short('n')
                    .long("nameserver")
                    .help("NS IP列表，多个使用英文逗号分隔，默认使用 Google Nameserver"),
            )
            .arg(
                Arg::new("no-wildcard")
                    .long("no-wildcard")
                    .action(ArgAction::SetFalse)
                    .help("关闭泛解析检查，默认开启"),
            )
            .arg(
                Arg::new("no-title")
                    .long("no-title")
                    .action(ArgAction::SetFalse)
                    .help("跳过存在域名的 title 获取，默认开启"),
            )
    }

    pub fn parse_cli_arguments(&mut self) -> &mut AppArgs {
        let mut cmd = self.build_command();
        let matches = cmd.get_matches_mut();

        // 取 target
        // TODO 校验 target 是否为正常的域名
        self.target = matches.get_one::<String>("target").unwrap().to_owned();

        // 取字典路径
        self.dict_path = if let Some(dict_path) = matches.get_one::<String>("dict") {
            // 用户提供了，检查文件是否存在，如果不存在直接退出
            if dict_path.is_empty() || Path::new(dict_path).exists() {
                Some(dict_path.to_owned())
            } else {
                cmd.error(
                    clap::error::ErrorKind::ValueValidation,
                    "字典文件不存在，请检查字典文件路径!",
                )
                .exit();
            }
        } else {
            None
        };
        // println!("self.dict_path: {:?}", self.dict_path);

        // 取 length ，判断 length 格式是否合法
        match self.parse_length(matches.get_one::<String>("length")) {
            Ok(length) => self.length = length,
            Err(e) => cmd.error(clap::error::ErrorKind::ValueValidation, e).exit(),
        }

        //  如果 length 和 dict 都没有指定，那么就返回错误
        if self.dict_path.is_none() && (self.length.0 == 0 && self.length.1 == 0) {
            cmd.error(
                clap::error::ErrorKind::MissingRequiredArgument,
                "dict 和 length 至少指定一个!",
            )
            .exit()
        };

        // 取 output
        self.output_path = matches
            .get_one::<String>("output")
            .map_or(format!("{}.txt", self.target), |it| it.to_owned());

        // 取 nameserver
        self.nameserver_list = matches
            .get_one::<String>("nameserver")
            .map_or(vec![], |it| it.split(',').collect::<Vec<&str>>())
            .iter()
            .map(|&it| it.to_owned())
            .collect::<Vec<String>>();

        // 取 task_count
        self.task_count = matches.get_one::<usize>("task-count").unwrap().to_owned();

        // 取 no-wildcard 和 no-title
        self.check_wildcard = matches.get_flag("no-wildcard");
        self.fetch_title = matches.get_flag("no-title");

        self
    }

    /// 解析 length 参数
    fn parse_length(&self, length: Option<&String>) -> Result<(usize, usize), &str> {
        if length.is_none() {
            return Ok((0, 0));
        }

        let length = length.unwrap();
        let length_part = length.split('-').collect::<Vec<_>>();
        // println!("length part: {:?}", length_part);
        match length_part.len() {
            1 => {
                if let Ok(_length) = length_part[0].parse::<usize>() {
                    Ok((_length, _length))
                } else {
                    Err("length参数有误，请检查长度参数-1!")
                }
            }
            2 => {
                if let (Ok(first), Ok(second)) = (
                    length_part[0].parse::<usize>(),
                    length_part[1].parse::<usize>(),
                ) {
                    println!("first: {}, second: {}", first, second);
                    if first != 0 && second != 0 && second > first {
                        return Ok((first, second));
                    }
                }
                Err("length参数有误，请检查长度参数-2!")
            }
            _ => Err("length参数有误，请检查长度参数-3!"),
        }
    }
}
