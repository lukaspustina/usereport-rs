# TODOs

## Before 0.1.0

* [x] Markdown Report
* [x] CLI with progressbar indication
* [x] Error handling in binary
* [x] Find a nice logger 
* [x] Profiles
    * [X] Config
    * [X] CLI opts: --show-profiles, --show-commands, --profile
* [X] Hostinfo
* [X] Config: Use defaults
* [X] Repetitions
* [X] Max Parallel
* [X] Refactor rendering using type class
* [X] File not found should not panic
* [X] Deprecate Report->AnalysisResult in favor of using Renderer directly
* [X] Commands should allow for arbitrary links for further information or actions
* [X] Command result should store command execution time
* [X] Rendering should allow for HTML
    * [X] Include failures in report
    * [X] Allow for generic handlebar based rendering via CLI
* [X] Templates
    * [X] Add Profile name
    * [X] crate info: Version
* [X] Show progressbar by default if terminal
* [ ] Correctly parse command args in ", e.g. sh -c "one | two"
* [ ] Add hbs helper which replaces \n with output format appropriate line break
* [ ] Create linux and macos configuration
* [ ] Preserve order of commands in output according to profile order
    * macOS
        * sw_vers
        * softwareupdate -l
        * http://www.brendangregg.com/USEmethod/use-macosx.html
    * Linux
        * https://www.cyberciti.biz/tips/top-linux-monitoring-tools.html
        * http://www.brendangregg.com/USEmethod/use-linux.html
* [X] Re-work API
    * [X] https://github.com/rust-lang/api-guidelines
    * [X] https://rust-lang.github.io/api-guidelines/flexibility.html#functions-minimize-assumptions-about-parameters-by-using-generics-c-generic
    * [X] https://deterministic.space/elegant-apis-in-rust.html
* [ ] Activate deny missing docs and add docs
    * [ ] https://rust-lang.github.io/api-guidelines/documentation.html

