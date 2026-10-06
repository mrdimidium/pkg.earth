// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: MPL-2.0

use crate::ui::components::{
    CodeBlock, DataTable, Disclosure, HeadingLevel, InstallStep, InstallSteps, Link, SectionHeading,
};
use maud::{Markup, Render, html};

pub struct ZigRelease {
    pub version: String,
    pub date: Option<String>,
    pub docs: Option<String>,
    pub std_docs: Option<String>,
    pub notes: Option<String>,
    pub files: Vec<ZigReleaseFile>,
}

pub struct ZigReleaseFile {
    pub filename: String,
    pub target: String,
}

pub struct GoRelease {
    pub version: String,
    pub stable: bool,
    pub files: Vec<GoReleaseFile>,
}

pub struct GoReleaseFile {
    pub filename: String,
}

pub struct IndexPage {
    pub zig: Vec<ZigRelease>,
    pub go: Vec<GoRelease>,
}

impl Render for IndexPage {
    fn render(&self) -> Markup {
        html! {
         h1 { "pkg.earth — tiny & opinionated packages mirror." }
         p { r#"This site provides a caching proxy for downloading Zig and Go installation files.
         It reduces load on upstream servers and makes your infrastructure more reliable by adding redundancy."# }
         p { "pkg.earth is open source software licensed under " (Link { href: "https://www.mozilla.org/MPL/2.0/", label: html! { "MPL-2.0" } }) ". " "Source code is hosted on " (Link { href: "https://gilti.dev/pkg.earth", label: html! { "Gilti" } }) " and mirrored on " (Link { href: "https://github.com/mrdimidium/pkg.earth", label: html! { "GitHub" } }) ". " "A list of dependency licenses " (Link { href: "/about/licenses", label: html! { "is available" } }) "." }
         (SectionHeading { level: HeadingLevel::Two, id: None, label: "Usage" })
         p { "Replace official download URLs with " (CodeBlock { text: "https://pkg.earth/{tool}/{filename}" }) ". Files are cached automatically after the first download." }
         (SectionHeading { level: HeadingLevel::Three, id: Some("zig"), label: "Zig" })
         @if !self.zig.is_empty() { (self.zig_disclosure()) }
         p { "Read more about community mirrors in the " (Link { href: "https://ziglang.org/download/community-mirrors/", label: html! { "blog post" } }) ". Information on how to deploy your own mirror is available " (Link { href: "https://github.com/ziglang/www.ziglang.org/blob/main/MIRRORS.md", label: html! { "in the documentation" } }) "." }
         p { "For simplicity, you can use tools like " (Link { href: "https://github.com/prantlf/zigup", label: html! { "prantlf/zigup" } }) " and " (Link { href: "https://github.com/mlugg/setup-zig", label: html! { "mlugg/setup-zig" } }) "." }
         p { "To install manually:" } (zig_install_steps())
         (SectionHeading { level: HeadingLevel::Three, id: Some("go"), label: "Go" }) @if !self.go.is_empty() { (self.go_disclosure()) }
         p { "To install manually:" } (go_install_steps())
         (SectionHeading { level: HeadingLevel::Two, id: None, label: "Privacy policy" })
         p { "This mirror is a non-profit project available on a voluntary basis. The author has no plans to fund it." }
         p { r#"Since the mirror is hosted on hardware, we collect access logs to combat bots and brute-force attacks.
 The logs are used for security purposes and load planning, are not shared with third parties,
 and are deleted after 30 days."# }
         p { "Third-party analytics systems are not used, same as client-side trackers." }
        }
    }
}
impl IndexPage {
    fn zig_disclosure(&self) -> Markup {
        let rows = html! { @for v in &self.zig { tr { td { (&v.version) } td { (v.date.as_deref().unwrap_or("-")) } td { @if let Some(url) = &v.docs { (Link { href: url, label: html! { "docs" } }) } " " @if let Some(url) = &v.std_docs { (Link { href: url, label: html! { "std" } }) } " " @if let Some(url) = &v.notes { (Link { href: url, label: html! { "notes" } }) } } td { @for file in &v.files { (Link { href: &format!("/zig/{}", file.filename), label: html! { (&file.target) } }) " " } } } } };
        Disclosure { summary: html! { "Available versions (" (self.zig.len()) ")" }, body: html! { p { "You can take actual minisig public key at " (Link { href: "https://ziglang.org/download/", label: html! { "ziglang.org/download" } }) "." } (DataTable { headers: &["Version", "Date", "Docs", "Targets"], rows }) } }.render()
    }
    fn go_disclosure(&self) -> Markup {
        let rows = html! { @for v in &self.go { tr { td { (&v.version) } td { @if v.stable { "✓" } } td { @for file in &v.files { (Link { href: &format!("/go/{}", file.filename), label: html! { (&file.filename) } }) " " } } } } };
        Disclosure { summary: html! { "Available versions (" (self.go.len()) ")" }, body: html! { p { "You can find available versions at " (Link { href: "https://go.dev/dl/", label: html! { "go.dev/dl" } }) "." } (DataTable { headers: &["Version", "Stable", "Files"], rows }) } }.render()
    }
}
fn zig_install_steps() -> InstallSteps<'static> {
    InstallSteps {
        steps: vec![
            InstallStep {
                label: "download zig dist file:",
                command: "wget https://pkg.earth/zig/zig-x86_64-linux-0.15.1.tar.xz",
            },
            InstallStep {
                label: "download zig minisig file:",
                command: "wget https://pkg.earth/zig/zig-x86_64-linux-0.15.1.tar.xz.minisig",
            },
            InstallStep {
                label: "check archive integrity:",
                command: "minisign -Vm zig-x86_64-linux-0.15.1.tar.xz -P RWSGOq2NVecA2UPNdBUZykf1CCb147pkmdtYxgb3Ti+JO/wCYvhbAb/U",
            },
            InstallStep {
                label: "unpack archive:",
                command: "tar -xf 'zig-x86_64-linux-0.15.1.tar.xz'",
            },
            InstallStep {
                label: "check installed zig:",
                command: "./zig-x86_64-linux-0.15.1/zig --version",
            },
        ],
    }
}

fn go_install_steps() -> InstallSteps<'static> {
    InstallSteps {
        steps: vec![
            InstallStep {
                label: "download go dist file:",
                command: "wget https://pkg.earth/go/go1.23.0.linux-amd64.tar.gz",
            },
            InstallStep {
                label: "download go sha256 file:",
                command: "wget https://pkg.earth/go/go1.23.0.linux-amd64.tar.gz.sha256",
            },
            InstallStep {
                label: "check archive integrity:",
                command: "sha256sum -c go1.23.0.linux-amd64.tar.gz.sha256",
            },
            InstallStep {
                label: "unpack archive:",
                command: "tar -xzf go1.23.0.linux-amd64.tar.gz",
            },
            InstallStep {
                label: "check installed go:",
                command: "./go/bin/go version",
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_data_omits_availability_disclosures() {
        let page = IndexPage {
            zig: Vec::new(),
            go: Vec::new(),
        }
        .render()
        .into_string();
        assert!(!page.contains("Available versions"));
    }

    #[test]
    fn release_data_is_escaped_and_composed_into_download_links() {
        let page = IndexPage {
            zig: vec![ZigRelease {
                version: "<version>".into(),
                date: None,
                docs: None,
                std_docs: None,
                notes: None,
                files: vec![ZigReleaseFile {
                    filename: "x&y".into(),
                    target: "<target>".into(),
                }],
            }],
            go: Vec::new(),
        }
        .render()
        .into_string();
        assert!(page.contains("&lt;version&gt;"));
        assert!(page.contains("/zig/x&amp;y"));
        assert!(page.contains("&lt;target&gt;"));
    }
}
