// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: MPL-2.0

use crate::ui::classes;
use crate::ui::components::{HeadingLevel, Link, SectionHeading};
use maud::{Markup, Render, html};

pub struct LicenseOverview {
    pub id: String,
    pub name: String,
    pub count: usize,
}

pub struct LicenseUse {
    pub name: String,
    pub version: String,
    pub repository: Option<String>,
}

pub struct LicenseEntry {
    pub id: String,
    pub name: String,
    pub text: String,
    pub first_of_kind: bool,
    pub used_by: Vec<LicenseUse>,
}

pub struct LicensesPage<'a> {
    pub project_license: &'a str,
    pub overview: Vec<LicenseOverview>,
    pub licenses: Vec<LicenseEntry>,
}

impl Render for LicensesPage<'_> {
    fn render(&self) -> Markup {
        html! {
         h1 { "Licenses" }

         (SectionHeading { level: HeadingLevel::Two, id: None, label: "pkg.earth" })

         p {
            "pkg.earth is licensed under the "
            b { "Mozilla Public License 2.0 (MPL-2.0)" } ". "
            "You may use, modify, and distribute the software. "
            "If you distribute modified covered files, you must make their source code available under MPL-2.0. "
            "The software is provided as-is, without warranty of any kind."
         }

         p {
            "Source code is hosted on "
            (Link { href: "https://gilti.dev/pkg.earth", label: html! { "Gilti" } }) " and mirrored on "
            (Link { href: "https://github.com/mrdimidium/pkg.earth", label: html! { "GitHub" } }) ". "
            "The full license text is included below."
         }

         pre class=(classes::licenses::LICENSE_TEXT) { (self.project_license) }

         (SectionHeading { level: HeadingLevel::Two, id: None, label: "Third party licenses" })

         ul {
            @for overview in &self.overview {
                li {
                    (Link { href: &format!("#{}", overview.id), label: html! { (&overview.name) } })
                    " (" (overview.count) ")"
                }
            }
         }

         @for (index, license) in self.licenses.iter().enumerate() {
            @if license.first_of_kind {
                (SectionHeading { level: HeadingLevel::Three, id: Some(&license.id), label: &license.name })

            }

            section class=(classes::licenses::LICENSE) {
                h4 id=(format!("{}-{index}", license.id)) { (&license.name) }
                p { "Used by:" }

                ul class=(classes::licenses::USED_BY) {
                    @for usage in &license.used_by {
                        li {
                            @if let Some(repository) = &usage.repository {
                                (Link { href: repository, label: html! { (&usage.name) "@" (&usage.version) } })
                            } @else {
                                (Link { href: &format!("https://crates.io/crates/{}", usage.name), label: html! { (&usage.name) "@" (&usage.version) } })
                            }
                        }
                    }
                }

                pre class=(classes::licenses::LICENSE_TEXT) { (&license.text) } }
         }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn license_page_preserves_structure_text_counts_and_repositories() {
        let page = LicensesPage {
            project_license: "full <project> license text",
            overview: vec![LicenseOverview {
                id: "mit".into(),
                name: "MIT".into(),
                count: 1,
            }],
            licenses: vec![LicenseEntry {
                id: "mit".into(),
                name: "MIT".into(),
                text: "<text>".into(),
                first_of_kind: true,
                used_by: vec![
                    LicenseUse {
                        name: "crate&name".into(),
                        version: "<version>".into(),
                        repository: None,
                    },
                    LicenseUse {
                        name: "repository-crate".into(),
                        version: "1.2.3".into(),
                        repository: Some("https://example.test/repository".into()),
                    },
                ],
            }],
        }
        .render()
        .into_string();
        assert!(page.contains("<h1>Licenses</h1>"));
        assert!(page.contains("<h3 class="));
        assert!(page.contains("id=\"mit\""));
        assert!(page.contains("#mit"));
        assert!(page.contains("MIT</a> (1)"));
        assert!(page.contains("full &lt;project&gt; license text"));
        assert!(page.contains("https://crates.io/crates/crate&amp;name"));
        assert!(page.contains("https://example.test/repository"));
        assert!(page.contains("&lt;text&gt;"));
    }
}
