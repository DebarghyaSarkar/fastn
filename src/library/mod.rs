mod fpm_dot_ftd;
mod http;
mod include;
mod sqlite;
mod toc;

#[derive(Debug)]
pub struct Library {
    pub config: fpm::Config,
    pub markdown: Option<(String, String)>,
    pub document_id: String,
    pub translated_data: fpm::TranslationData,
}

impl ftd::p2::Library for Library {
    fn get(&self, name: &str, doc: &ftd::p2::TDoc) -> Option<String> {
        // Standard libraries
        if name == "fpm" {
            return Some(fpm_dot_ftd::get(self));
        }
        if name == "fpm-lib" {
            return Some(fpm::fpm_lib_ftd().to_string());
        }
        return if let Some(r) = get_for_package_config(name, &self.config.package, self) {
            Some(r)
        } else {
            for package in &get_root_package_for_path(doc.name, &self.config.package, false) {
                if let Some(resp) = get_for_package_config(name, package, self) {
                    return Some(resp);
                };
            }
            None
        };

        fn get_for_package_config(
            name: &str,
            package: &fpm::Package,
            lib: &fpm::Library,
        ) -> Option<String> {
            if name.starts_with(package.name.as_str()) {
                if let Some(r) = get_data_from_package(name, package, lib) {
                    return Some(r);
                }
            }
            // If package == lib.config.package => Current iteration for the top most package
            // Root package evaluation
            if package.name == lib.config.package.name {
                if let Some(translation_of_package) = lib.config.package.translation_of.as_ref() {
                    // Index document can be accessed from the package name directly. For others `/` is required to be certain.
                    // vivekanand-hi -> vivekanand-hi-hi This is wrong. That's why we ensure a strict `/` check or a full name match
                    let new_name = if translation_of_package.name.as_str().eq(name) {
                        package.name.clone()
                    } else {
                        name.replacen(
                            format!("{}/", translation_of_package.name.as_str()).as_str(),
                            format!("{}/", package.name.as_str()).as_str(),
                            1,
                        )
                    };

                    if let Some(resp) = get_data_from_package(new_name.as_str(), package, lib) {
                        return Some(resp);
                    }
                }
            }

            // Check the translation of the package
            if let Some(translation_of_package) = package.translation_of.as_ref() {
                if let Some(resp) = get_for_package_config(
                    name.replacen(
                        package.name.as_str(),
                        translation_of_package.name.as_str(),
                        1,
                    )
                    .as_str(),
                    translation_of_package,
                    lib,
                ) {
                    return Some(resp);
                }
            }

            if let Some(r) = get_from_all_dependencies(name, package, lib) {
                return Some(r);
            }
            None
        }
        fn get_root_package_for_path(
            name: &str,
            package: &fpm::Package,
            include_self: bool,
        ) -> Vec<fpm::Package> {
            if name.starts_with(package.name.as_str()) {
                if include_self {
                    vec![package.to_owned()]
                } else {
                    vec![]
                }
            } else {
                let mut resp = vec![];
                for dep in &package.dependencies {
                    if let Some(unaliased_name) = dep.unaliased_name(name) {
                        resp.extend(get_root_package_for_path(
                            unaliased_name.as_str(),
                            &dep.package,
                            false,
                        ));
                        resp.push(dep.package.clone())
                    }
                }
                resp
            }
        }

        fn get_from_all_dependencies(
            name: &str,
            package: &fpm::Package,
            lib: &fpm::Library,
            // evaluated_packages: &mut Vec<String>,
        ) -> Option<String> {
            for dep in &package.get_flattened_dependencies() {
                if let Some(non_aliased_name) = dep.unaliased_name(name) {
                    if non_aliased_name.starts_with(dep.package.name.as_str()) {
                        if let Some(resp) =
                            get_from_dependency(non_aliased_name.as_str(), &dep.package, lib)
                        {
                            return Some(resp);
                        };
                    }
                }
            }
            None
        }
        fn get_from_dependency(
            name: &str,
            from_package: &fpm::Package,
            lib: &fpm::Library,
        ) -> Option<String> {
            // TODO: Here the library needs to be evaluated for this particular package
            // Right now the solution works by recursively looking for the package in the dependency tree
            // Ideally we should also know the library definition of a particular package
            if let Some(resp_body) = get_data_from_package(name, from_package, lib) {
                return Some(resp_body);
            }
            None
        }

        fn get_file_from_location(base_path: &camino::Utf8PathBuf, name: &str) -> Option<String> {
            let os_name = name
                .trim_start_matches('/')
                .trim_end_matches('/')
                .replace("/", std::path::MAIN_SEPARATOR.to_string().as_str());
            if let Ok(v) = std::fs::read_to_string(base_path.join(format!("{}.ftd", os_name))) {
                return Some(v);
            }
            if let Ok(v) = std::fs::read_to_string(base_path.join(os_name).join("index.ftd")) {
                return Some(v);
            }
            None
        }

        fn get_data_from_package(
            name: &str,
            package: &fpm::Package,
            lib: &Library,
        ) -> Option<String> {
            let path = if let Some(package_fpm_path) = &package.fpm_path {
                package_fpm_path.parent()?.to_owned()
            } else if package.name.eq(&lib.config.package.name) {
                lib.config.root.clone()
            } else {
                lib.config.packages_root.clone().join(package.name.as_str())
            };
            // Explicit check for the current package.
            if name.starts_with(&package.name.as_str()) {
                let new_name = name.replacen(&package.name.as_str(), "", 1);
                if new_name.as_str().trim_start_matches('/') == "assets" {
                    // Virtual document for getting the assets
                    return Some(get_assets_doc_for_package(package));
                } else if let Some(body) = get_file_from_location(&path, new_name.as_str()) {
                    return Some(package.get_prefixed_body(body.as_str(), name, false));
                }
            }
            None
        }

        fn get_assets_doc_for_package(package: &fpm::Package) -> String {
            use itertools::Itertools;

            let (font_record, fonts) = package
                .fonts
                .iter()
                .unique_by(|font| font.name.as_str())
                .collect_vec()
                .iter()
                .fold(
                    (
                        String::from("-- record font:"),
                        String::from("-- font fonts:"),
                    ),
                    |(record_accumulator, instance_accumulator), font| {
                        (
                            format!(
                                "{pre}\nstring {font_var_name}:",
                                pre = record_accumulator,
                                font_var_name = font.name.as_str(),
                            ),
                            format!(
                                "{pre}\n{font_var_name}: {font_var_val}",
                                pre = instance_accumulator,
                                font_var_name = font.name.as_str(),
                                font_var_val = font.html_name(package.name.as_str())
                            ),
                        )
                    },
                );
            format!(
                indoc::indoc! {"
                    {font_record}
                    {fonts}
                "},
                font_record = font_record,
                fonts = fonts
            )
        }
    }

    fn process(
        &self,
        section: &ftd::p1::Section,
        doc: &ftd::p2::TDoc,
    ) -> ftd::p1::Result<ftd::Value> {
        match section
            .header
            .str(doc.name, section.line_number, "$processor$")?
        {
            // "toc" => fpm::library::toc::processor(section, doc),
            "http" => fpm::library::http::processor(section, doc),
            "package-query" => fpm::library::sqlite::processor(section, doc, &self.config),
            "toc" => fpm::library::toc::processor(section, doc, &self.config),
            "include" => fpm::library::include::processor(section, doc, &self.config),
            t => unimplemented!("$processor$: {} is not implemented yet", t),
        }
    }
}

#[derive(Default)]
pub struct FPMLibrary {}

impl ftd::p2::Library for FPMLibrary {
    fn get(&self, name: &str, _doc: &ftd::p2::TDoc) -> Option<String> {
        if name == "fpm" {
            return Some(format!(
                "{}\n\n-- optional package-data package:\n",
                fpm::fpm_ftd()
            ));
        } else {
            std::fs::read_to_string(format!("./{}.ftd", name)).ok()
        }
    }
}
