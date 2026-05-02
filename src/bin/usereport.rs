fn main() -> miette::Result<()> {
    human_panic::setup_panic!();
    miette::set_hook(Box::new(|_| Box::new(miette::MietteHandlerOpts::new().build()))).ok();
    usereport::cli::main()
}
