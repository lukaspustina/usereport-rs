# TODOs

## Before 0.1

* [x] Markdown Report
* [x] CLI with progressbar indication
* [x] Error handling in binary
* [x] Find a nice logger 
* Profiles
    * [X] Config
    * [X] CLI opts: --show-profiles, --show-commands, --profile
* [X] Hostinfo
* [ ] Repetitions
* [ ] Max Parallel
* [ ] Create linux and macos configuration
    * macOS
        * sw_vers
        * softwareupdate -l
        * http://www.brendangregg.com/USEmethod/use-macosx.html
    * Linux
        * https://www.cyberciti.biz/tips/top-linux-monitoring-tools.html
        * http://www.brendangregg.com/USEmethod/use-linux.html
* [ ] Include failures in report

## Before 1.0

* Config: Use defaults
* Use a abstract type to hold results
    * Include Host information etc. that are currently in Report (maybe)
    * Repeat measurements
* Commands should allow for arbitary links for further information or actions
* Command result should store command execution time
* Command result should allow easier access -- cf. command args in rendering
* Rendering should allow for HTML

