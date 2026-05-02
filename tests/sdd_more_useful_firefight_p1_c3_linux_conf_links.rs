//! SDD more-useful-firefight Phase 1, C3.
//! GIVEN linux.conf WHEN all links URL strings are collected via TOML parse
//! THEN none starts with "http://" and tcp_mem links contain "man7/tcp.7.html".
#![cfg(feature = "bin")]

#[test]
fn linux_conf_links_all_https() {
    let toml_src = include_str!("../contrib/linux.conf");
    let val: toml::Value = toml::from_str(toml_src).expect("linux.conf must be valid TOML");
    let commands = val["command"].as_array().expect("command array");
    let mut found_http = Vec::new();
    for cmd in commands {
        let name = cmd["name"].as_str().unwrap_or("<unknown>");
        if let Some(links) = cmd.get("links").and_then(|l| l.as_array()) {
            for link in links {
                if let Some(url) = link.get("url").and_then(|u| u.as_str()) {
                    if url.starts_with("http://") {
                        found_http.push(format!("{}: {}", name, url));
                    }
                }
            }
        }
    }
    assert!(
        found_http.is_empty(),
        "found http:// links (should all be https://): {:?}",
        found_http
    );
}

#[test]
fn linux_conf_tcp_mem_link_correct() {
    let toml_src = include_str!("../contrib/linux.conf");
    let val: toml::Value = toml::from_str(toml_src).expect("linux.conf must be valid TOML");
    let commands = val["command"].as_array().expect("command array");
    let tcp_mem = commands
        .iter()
        .find(|c| c["name"].as_str() == Some("tcp_mem"))
        .expect("tcp_mem command must exist");
    let links = tcp_mem["links"].as_array().expect("tcp_mem must have links");
    let has_tcp_man = links.iter().any(|l| {
        l.get("url")
            .and_then(|u| u.as_str())
            .map(|u| u.contains("man7/tcp.7.html"))
            .unwrap_or(false)
    });
    assert!(has_tcp_man, "tcp_mem must have a link containing 'man7/tcp.7.html'");
}
