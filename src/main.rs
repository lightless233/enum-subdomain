use args::AppArgs;

mod args;

fn main() {
    let mut app_args = AppArgs::default();
    app_args.parse_cli_arguments();

    println!("app_args: {:?}", app_args);
}
