fn cached_parse(
    id: &str,
    source: &str,
    line_number: usize,
) -> ftd::interpreter::Result<ftd::interpreter::ParsedDocument> {
    #[derive(serde::Deserialize, serde::Serialize)]
    struct C {
        hash: String,
        doc: ftd::interpreter::ParsedDocument,
    }

    let hash = fastn_core::utils::generate_hash(source);

    if let Some(c) = fastn_core::utils::get_cached::<C>(id) {
        if c.hash == hash {
            tracing::debug!("cache hit");
            return Ok(c.doc);
        }
        tracing::debug!("cached hash mismatch");
    } else {
        tracing::debug!("cached miss");
    }

    let doc = ftd::interpreter::ParsedDocument::parse_with_line_number(id, source, line_number)?;
    fastn_core::utils::cache_it(id, C { doc, hash }).map(|v| v.doc)
}

#[tracing::instrument(skip_all)]
pub async fn interpret_helper<'a>(
    name: &str,
    source: &str,
    lib: &'a mut fastn_core::Library2022,
    base_url: &str,
    download_assets: bool,
    line_number: usize,
) -> ftd::interpreter::Result<ftd::interpreter::Document> {
    tracing::info!(document = name);
    let doc = cached_parse(name, source, line_number)?;
    let mut s = ftd::interpreter::interpret_with_line_number(name, doc, line_number)?;
    lib.module_package_map.insert(
        name.trim_matches('/').to_string(),
        lib.config.package.name.to_string(),
    );
    let document;
    loop {
        match s {
            ftd::interpreter::Interpreter::Done { document: doc } => {
                document = doc;
                break;
            }
            ftd::interpreter::Interpreter::StuckOnImport {
                module,
                state: mut st,
                caller_module,
            } => {
                let (source, path, foreign_variable, foreign_function, ignore_line_numbers) =
                    resolve_import_2022(lib, &mut st, module.as_str(), caller_module.as_str())
                        .await?;
                lib.config.dependencies_during_render.push(path);
                let doc = cached_parse(module.as_str(), source.as_str(), ignore_line_numbers)?;
                s = st.continue_after_import(
                    module.as_str(),
                    doc,
                    foreign_variable,
                    foreign_function,
                    ignore_line_numbers,
                )?;
            }
            ftd::interpreter::Interpreter::StuckOnProcessor {
                state,
                ast,
                module,
                processor,
                ..
            } => {
                let doc = state.get_current_processing_module().ok_or(
                    ftd::interpreter::Error::ValueNotFound {
                        doc_id: module,
                        line_number: ast.line_number(),
                        message: "Cannot find the module".to_string(),
                    },
                )?;
                let line_number = ast.line_number();
                let value = lib
                    .process(
                        ast.clone(),
                        processor,
                        &mut state.tdoc(doc.as_str(), line_number)?,
                    )
                    .await?;
                s = state.continue_after_processor(value, ast)?;
            }
            ftd::interpreter::Interpreter::StuckOnForeignVariable {
                state,
                module,
                variable,
                caller_module,
            } => {
                let value = resolve_foreign_variable2022(
                    variable.as_str(),
                    module.as_str(),
                    lib,
                    base_url,
                    download_assets,
                    caller_module.as_str(),
                )
                .await?;
                s = state.continue_after_variable(module.as_str(), variable.as_str(), value)?;
            }
        }
    }
    Ok(document)
}

pub async fn resolve_import<'a>(
    lib: &'a mut fastn_core::Library2,
    state: &mut ftd::ftd2021::InterpreterState,
    module: &str,
) -> ftd::ftd2021::p1::Result<String> {
    lib.packages_under_process
        .truncate(state.document_stack.len());
    let current_package = lib.get_current_package()?;
    let source = if module.eq("fastn/time") {
        state.add_foreign_variable_prefix(module, vec![module.to_string()]);
        lib.push_package_under_process(&current_package).await?;
        "".to_string()
    } else if module.ends_with("assets") {
        state.add_foreign_variable_prefix(module, vec![format!("{}#files", module)]);

        if module.starts_with(current_package.name.as_str()) {
            lib.push_package_under_process(&current_package).await?;
            lib.get_current_package()?
                .get_font_ftd()
                .unwrap_or_default()
        } else {
            let mut font_ftd = "".to_string();
            for (alias, package) in current_package.aliases() {
                if module.starts_with(alias) {
                    lib.push_package_under_process(package).await?;
                    font_ftd = lib
                        .config
                        .all_packages
                        .borrow()
                        .get(package.name.as_str())
                        .unwrap()
                        .get_font_ftd()
                        .unwrap_or_default();
                    break;
                }
            }
            font_ftd
        }
    } else {
        lib.get_with_result(module).await?
    };

    Ok(source)
}

// source, foreign_variable, foreign_function
pub async fn resolve_import_2022<'a>(
    lib: &'a mut fastn_core::Library2022,
    _state: &mut ftd::interpreter::InterpreterState,
    module: &str,
    caller_module: &str,
) -> ftd::interpreter::Result<(String, String, Vec<String>, Vec<String>, usize)> {
    let current_package = lib.get_current_package(caller_module)?;
    let source = if module.eq("fastn/time") {
        (
            "".to_string(),
            "$fastn$/time.ftd".to_string(),
            vec!["time".to_string()],
            vec![],
            0,
        )
    } else if module.eq("fastn/processors") {
        (
            fastn_core::processor_ftd().to_string(),
            "$fastn$/processors.ftd".to_string(),
            vec![],
            vec![
                "figma-typo-token".to_string(),
                "figma-cs-token".to_string(),
                "figma-cs-token-old".to_string(),
                "http".to_string(),
                "get-data".to_string(),
                "toc".to_string(),
                "sitemap".to_string(),
                "full-sitemap".to_string(),
                "request-data".to_string(),
                "document-readers".to_string(),
                "document-writers".to_string(),
                "user-groups".to_string(),
                "user-group-by-id".to_string(),
                "get-identities".to_string(),
                "document-id".to_string(),
                "document-full-id".to_string(),
                "document-suffix".to_string(),
                "document-name".to_string(),
                "user-details".to_string(),
                "fastn-apps".to_string(),
                "is-reader".to_string(),
                "package-query".to_string(),
                "pg".to_string(),
                "package-tree".to_string(),
                "fetch-file".to_string(),
                "query".to_string(),
            ],
            0,
        )
    } else if module.ends_with("assets") {
        let foreign_variable = vec!["files".to_string()];

        if module.starts_with(current_package.name.as_str()) {
            (
                current_package.get_font_ftd().unwrap_or_default(),
                format!("{name}/-/assets.ftd", name = current_package.name),
                foreign_variable,
                vec![],
                0,
            )
        } else {
            let mut font_ftd = "".to_string();
            let mut path = "".to_string();
            for (alias, package) in current_package.aliases() {
                if module.starts_with(alias) {
                    lib.push_package_under_process(module, package).await?;
                    font_ftd = lib
                        .config
                        .all_packages
                        .borrow()
                        .get(package.name.as_str())
                        .unwrap()
                        .get_font_ftd()
                        .unwrap_or_default();
                    path = format!("{name}/-/fonts.ftd", name = package.name);
                    break;
                }
            }
            (font_ftd, path, foreign_variable, vec![], 0)
        }
    } else {
        let (content, path, ignore_line_numbers) =
            lib.get_with_result(module, caller_module).await?;
        (
            content,
            path,
            vec![],
            vec![
                "figma-typo-token".to_string(),
                "figma-cs-token".to_string(),
                "figma-cs-token-old".to_string(),
                "http".to_string(),
                "package-query".to_string(),
                "pg".to_string(),
                "toc".to_string(),
                "include".to_string(),
                "get-data".to_string(),
                "sitemap".to_string(),
                "full-sitemap".to_string(),
                "user-groups".to_string(),
                "document-readers".to_string(),
                "document-writers".to_string(),
                "user-group-by-id".to_string(),
                "get-identities".to_string(),
                "document-id".to_string(),
                "document-full-id".to_string(),
                "document-name".to_string(),
                "document-suffix".to_string(),
                "package-id".to_string(),
                "package-tree".to_string(),
                "fetch-file".to_string(),
                "get-version-data".to_string(),
                "cr-meta".to_string(),
                "request-data".to_string(),
                "user-details".to_string(),
                "fastn-apps".to_string(),
                "is-reader".to_string(),
            ],
            ignore_line_numbers,
        )
    };
    Ok(source)
}

#[tracing::instrument(name = "fastn_core::stuck-on-foreign-variable", err, skip(lib))]
pub async fn resolve_foreign_variable2022(
    variable: &str,
    doc_name: &str,
    lib: &mut fastn_core::Library2022,
    base_url: &str,
    download_assets: bool,
    caller_module: &str,
) -> ftd::interpreter::Result<ftd::interpreter::Value> {
    tracing::info!(doc = doc_name, var = variable);
    let package = lib.get_current_package(caller_module)?;
    if let Ok(value) = resolve_ftd_foreign_variable_2022(variable, doc_name) {
        return Ok(value);
    }

    if variable.starts_with("files.") {
        let files = variable.trim_start_matches("files.").to_string();
        let package_name = doc_name.trim_end_matches("/assets").to_string();

        if package.name.eq(&package_name) {
            if let Ok(value) = get_assets_value(
                doc_name,
                &package,
                files.as_str(),
                lib,
                base_url,
                download_assets,
            )
            .await
            {
                return Ok(value);
            }
        }
        for (alias, package) in package.aliases() {
            if alias.eq(&package_name) {
                lib.push_package_under_process(doc_name, package).await?;
                let package = lib
                    .config
                    .all_packages
                    .borrow()
                    .get(package.name.as_str())
                    .unwrap_or(package)
                    .to_owned();
                if let Ok(value) = get_assets_value(
                    doc_name,
                    &package,
                    files.as_str(),
                    lib,
                    base_url,
                    download_assets,
                )
                .await
                {
                    return Ok(value);
                }
            }
        }
    }

    return ftd::interpreter::utils::e2(format!("{} not found 2", variable).as_str(), doc_name, 0);

    async fn get_assets_value(
        module: &str,
        package: &fastn_core::Package,
        files: &str,
        lib: &mut fastn_core::Library2022,
        base_url: &str,
        download_assets: bool, // true: in case of `fastn build`
    ) -> ftd::ftd2021::p1::Result<ftd::interpreter::Value> {
        lib.push_package_under_process(module, package).await?;
        let _base_url = base_url.trim_end_matches('/');
        let mut files = files.to_string();
        let light = {
            if let Some(f) = files.strip_suffix(".light") {
                files = f.to_string();
                true
            } else {
                false
            }
        };
        let dark = {
            if light {
                false
            } else if let Some(f) = files.strip_suffix(".dark") {
                files = f.to_string();
                true
            } else {
                false
            }
        };

        match files.rsplit_once('.') {
            Some((file, ext))
                if mime_guess::MimeGuess::from_ext(ext)
                    .first_or_octet_stream()
                    .to_string()
                    .starts_with("image/") =>
            {
                let light_mode = format!("/-/{}/{}.{}", package.name, file.replace('.', "/"), ext)
                    .trim_start_matches('/')
                    .to_string();

                let light_path = format!("{}.{}", file.replace('.', "/"), ext);
                if download_assets
                    && !lib
                        .config
                        .downloaded_assets
                        .contains_key(&format!("{}/{}", package.name, light_path))
                {
                    let start = std::time::Instant::now();
                    let light = package
                        .resolve_by_file_name(light_path.as_str(), None, false)
                        .await
                        .map_err(|e| ftd::ftd2021::p1::Error::ParseError {
                            message: e.to_string(),
                            doc_id: lib.document_id.to_string(),
                            line_number: 0,
                        })?;
                    print!("Processing {}/{} ... ", package.name.as_str(), light_path);
                    fastn_core::utils::write(
                        &lib.config.build_dir().join("-").join(package.name.as_str()),
                        light_path.as_str(),
                        light.as_slice(),
                    )
                    .await
                    .map_err(|e| ftd::ftd2021::p1::Error::ParseError {
                        message: e.to_string(),
                        doc_id: lib.document_id.to_string(),
                        line_number: 0,
                    })?;
                    lib.config.downloaded_assets.insert(
                        format!("{}/{}", package.name, light_path),
                        light_mode.to_string(),
                    );
                    fastn_core::utils::print_end(
                        format!("Processed {}/{}", package.name.as_str(), light_path).as_str(),
                        start,
                    );
                }

                if light {
                    return Ok(ftd::interpreter::Value::String {
                        text: light_mode.trim_start_matches('/').to_string(),
                    });
                }

                let mut dark_mode = if file.ends_with("-dark") {
                    light_mode.clone()
                } else {
                    format!(
                        "/-/{}/{}-dark.{}",
                        package.name,
                        file.replace('.', "/"),
                        ext
                    )
                    .trim_start_matches('/')
                    .to_string()
                };

                let dark_path = format!("{}-dark.{}", file.replace('.', "/"), ext);
                if download_assets && !file.ends_with("-dark") {
                    let start = std::time::Instant::now();
                    if let Some(dark) = lib
                        .config
                        .downloaded_assets
                        .get(&format!("{}/{}", package.name, dark_path))
                    {
                        dark_mode = dark.to_string();
                    } else if let Ok(dark) = package
                        .resolve_by_file_name(dark_path.as_str(), None, false)
                        .await
                    {
                        print!("Processing {}/{} ... ", package.name.as_str(), dark_path);
                        fastn_core::utils::write(
                            &lib.config.build_dir().join("-").join(package.name.as_str()),
                            dark_path.as_str(),
                            dark.as_slice(),
                        )
                        .await
                        .map_err(|e| {
                            ftd::ftd2021::p1::Error::ParseError {
                                message: e.to_string(),
                                doc_id: lib.document_id.to_string(),
                                line_number: 0,
                            }
                        })?;
                        fastn_core::utils::print_end(
                            format!("Processed {}/{}", package.name.as_str(), dark_path).as_str(),
                            start,
                        );
                    } else {
                        dark_mode = light_mode.clone();
                    }
                    lib.config.downloaded_assets.insert(
                        format!("{}/{}", package.name, dark_path),
                        dark_mode.to_string(),
                    );
                }

                if dark {
                    return Ok(ftd::interpreter::Value::String {
                        text: dark_mode.trim_start_matches('/').to_string(),
                    });
                }
                #[allow(deprecated)]
                Ok(ftd::interpreter::Value::Record {
                    name: "ftd#image-src".to_string(),
                    fields: std::array::IntoIter::new([
                        (
                            "light".to_string(),
                            ftd::interpreter::PropertyValue::Value {
                                value: ftd::interpreter::Value::String { text: light_mode },
                                is_mutable: false,
                                line_number: 0,
                            },
                        ),
                        (
                            "dark".to_string(),
                            ftd::interpreter::PropertyValue::Value {
                                value: ftd::interpreter::Value::String { text: dark_mode },
                                is_mutable: false,
                                line_number: 0,
                            },
                        ),
                    ])
                    .collect(),
                })
            }
            Some((file, ext)) => {
                download(
                    lib,
                    download_assets,
                    package,
                    format!("{}.{}", file.replace('.', "/"), ext).as_str(),
                )
                .await?;
                Ok(ftd::interpreter::Value::String {
                    text: format!("-/{}/{}.{}", package.name, file.replace('.', "/"), ext),
                })
            }
            None => {
                download(lib, download_assets, package, files.as_str()).await?;
                Ok(ftd::interpreter::Value::String {
                    text: format!("-/{}/{}", package.name, files),
                })
            }
        }
    }
}

async fn download(
    lib: &mut fastn_core::Library2022,
    download_assets: bool,
    package: &fastn_core::Package,
    path: &str,
) -> ftd::ftd2021::p1::Result<()> {
    if download_assets
        && !lib
            .config
            .downloaded_assets
            .contains_key(&format!("{}/{}", package.name, path))
    {
        let start = std::time::Instant::now();
        let data = package
            .resolve_by_file_name(path, None, false)
            .await
            .map_err(|e| ftd::ftd2021::p1::Error::ParseError {
                message: e.to_string(),
                doc_id: lib.document_id.to_string(),
                line_number: 0,
            })?;
        print!("Processing {}/{} ... ", package.name, path);
        fastn_core::utils::write(
            &lib.config.build_dir().join("-").join(package.name.as_str()),
            path,
            data.as_slice(),
        )
        .await
        .map_err(|e| ftd::ftd2021::p1::Error::ParseError {
            message: e.to_string(),
            doc_id: lib.document_id.to_string(),
            line_number: 0,
        })?;
        lib.config.downloaded_assets.insert(
            format!("{}/{}", package.name, path),
            format!("-/{}/{}", package.name, path),
        );
        fastn_core::utils::print_end(
            format!("Processed {}/{}", package.name, path).as_str(),
            start,
        );
    }

    Ok(())
}

pub async fn resolve_foreign_variable2(
    variable: &str,
    doc_name: &str,
    state: &ftd::ftd2021::InterpreterState,
    lib: &mut fastn_core::Library2,
    base_url: &str,
    download_assets: bool,
) -> ftd::ftd2021::p1::Result<ftd::Value> {
    lib.packages_under_process
        .truncate(state.document_stack.len());
    let package = lib.get_current_package()?;
    if let Ok(value) = resolve_ftd_foreign_variable(variable, doc_name) {
        return Ok(value);
    }

    if let Some((package_name, files)) = variable.split_once("/assets#files.") {
        if package.name.eq(package_name) {
            if let Ok(value) =
                get_assets_value(&package, files, lib, base_url, download_assets).await
            {
                return Ok(value);
            }
        }
        for (alias, package) in package.aliases() {
            if alias.eq(package_name) {
                if let Ok(value) =
                    get_assets_value(package, files, lib, base_url, download_assets).await
                {
                    return Ok(value);
                }
            }
        }
    }

    return ftd::ftd2021::p2::utils::e2(format!("{} not found 2", variable).as_str(), doc_name, 0);

    async fn get_assets_value(
        package: &fastn_core::Package,
        files: &str,
        lib: &mut fastn_core::Library2,
        base_url: &str,
        download_assets: bool, // true: in case of `fastn build`
    ) -> ftd::ftd2021::p1::Result<ftd::Value> {
        lib.push_package_under_process(package).await?;
        let base_url = base_url.trim_end_matches('/');
        let mut files = files.to_string();
        let light = {
            if let Some(f) = files.strip_suffix(".light") {
                files = f.to_string();
                true
            } else {
                false
            }
        };
        let dark = {
            if light {
                false
            } else if let Some(f) = files.strip_suffix(".dark") {
                files = f.to_string();
                true
            } else {
                false
            }
        };

        match files.rsplit_once('.') {
            Some((file, ext))
                if mime_guess::MimeGuess::from_ext(ext)
                    .first_or_octet_stream()
                    .to_string()
                    .starts_with("image/") =>
            {
                let light_mode = format!(
                    "{base_url}/-/{}/{}.{}",
                    package.name,
                    file.replace('.', "/"),
                    ext
                )
                .trim_start_matches('/')
                .to_string();

                let light_path = format!("{}.{}", file.replace('.', "/"), ext);
                if download_assets
                    && !lib
                        .config
                        .downloaded_assets
                        .contains_key(&format!("{}/{}", package.name, light_path))
                {
                    let start = std::time::Instant::now();
                    let light = package
                        .resolve_by_file_name(light_path.as_str(), None, false)
                        .await
                        .map_err(|e| ftd::ftd2021::p1::Error::ParseError {
                            message: e.to_string(),
                            doc_id: lib.document_id.to_string(),
                            line_number: 0,
                        })?;
                    print!("Processing {}/{} ... ", package.name.as_str(), light_path);
                    fastn_core::utils::write(
                        &lib.config.build_dir().join("-").join(package.name.as_str()),
                        light_path.as_str(),
                        light.as_slice(),
                    )
                    .await
                    .map_err(|e| ftd::ftd2021::p1::Error::ParseError {
                        message: e.to_string(),
                        doc_id: lib.document_id.to_string(),
                        line_number: 0,
                    })?;
                    lib.config.downloaded_assets.insert(
                        format!("{}/{}", package.name, light_path),
                        light_mode.to_string(),
                    );
                    fastn_core::utils::print_end(
                        format!("Processed {}/{}", package.name.as_str(), light_path).as_str(),
                        start,
                    );
                }

                if light {
                    return Ok(ftd::Value::String {
                        text: light_mode,
                        source: ftd::TextSource::Header,
                    });
                }

                let mut dark_mode = if file.ends_with("-dark") {
                    light_mode.clone()
                } else {
                    format!(
                        "{base_url}/-/{}/{}-dark.{}",
                        package.name,
                        file.replace('.', "/"),
                        ext
                    )
                    .trim_start_matches('/')
                    .to_string()
                };

                let dark_path = format!("{}-dark.{}", file.replace('.', "/"), ext);
                if download_assets && !file.ends_with("-dark") {
                    let start = std::time::Instant::now();
                    if let Some(dark) = lib
                        .config
                        .downloaded_assets
                        .get(&format!("{}/{}", package.name, dark_path))
                    {
                        dark_mode = dark.to_string();
                    } else if let Ok(dark) = package
                        .resolve_by_file_name(dark_path.as_str(), None, false)
                        .await
                    {
                        print!("Processing {}/{} ... ", package.name.as_str(), dark_path);
                        fastn_core::utils::write(
                            &lib.config.build_dir().join("-").join(package.name.as_str()),
                            dark_path.as_str(),
                            dark.as_slice(),
                        )
                        .await
                        .map_err(|e| {
                            ftd::ftd2021::p1::Error::ParseError {
                                message: e.to_string(),
                                doc_id: lib.document_id.to_string(),
                                line_number: 0,
                            }
                        })?;
                        fastn_core::utils::print_end(
                            format!("Processed {}/{}", package.name.as_str(), dark_path).as_str(),
                            start,
                        );
                    } else {
                        dark_mode = light_mode.clone();
                    }
                    lib.config.downloaded_assets.insert(
                        format!("{}/{}", package.name, dark_path),
                        dark_mode.to_string(),
                    );
                }

                if dark {
                    return Ok(ftd::Value::String {
                        text: dark_mode,
                        source: ftd::TextSource::Header,
                    });
                }
                #[allow(deprecated)]
                Ok(ftd::Value::Record {
                    name: "ftd#image-src".to_string(),
                    fields: std::array::IntoIter::new([
                        (
                            "light".to_string(),
                            ftd::PropertyValue::Value {
                                value: ftd::Value::String {
                                    text: light_mode,
                                    source: ftd::TextSource::Header,
                                },
                            },
                        ),
                        (
                            "dark".to_string(),
                            ftd::PropertyValue::Value {
                                value: ftd::Value::String {
                                    text: dark_mode,
                                    source: ftd::TextSource::Header,
                                },
                            },
                        ),
                    ])
                    .collect(),
                })
            }
            Some((file, ext)) => Ok(ftd::Value::String {
                text: format!("-/{}/{}.{}", package.name, file.replace('.', "/"), ext),
                source: ftd::TextSource::Header,
            }),
            None => Ok(ftd::Value::String {
                text: format!("-/{}/{}", package.name, files),
                source: ftd::TextSource::Header,
            }),
        }
    }
}

// No need to make async since this is pure.
pub fn parse_ftd(
    name: &str,
    source: &str,
    lib: &fastn_core::FastnLibrary,
) -> ftd::ftd2021::p1::Result<ftd::ftd2021::p2::Document> {
    let mut s = ftd::ftd2021::interpret(name, source, &None)?;
    let document;
    loop {
        match s {
            ftd::ftd2021::Interpreter::Done { document: doc } => {
                document = doc;
                break;
            }
            ftd::ftd2021::Interpreter::StuckOnProcessor { .. } => {
                unimplemented!()
            }
            ftd::ftd2021::Interpreter::StuckOnImport { module, state: st } => {
                let source = lib.get_with_result(
                    module.as_str(),
                    &st.tdoc(&mut Default::default(), &mut Default::default()),
                )?;
                s = st.continue_after_import(module.as_str(), source.as_str())?;
            }
            ftd::ftd2021::Interpreter::StuckOnForeignVariable { .. } => {
                unimplemented!()
            }
            ftd::ftd2021::Interpreter::CheckID { .. } => {
                // No config in fastn_core::FastnLibrary ignoring processing terms here
                unimplemented!()
            }
        }
    }
    Ok(document)
}

fn resolve_ftd_foreign_variable(
    variable: &str,
    doc_name: &str,
) -> ftd::ftd2021::p1::Result<ftd::Value> {
    match variable.strip_prefix("fastn/time#") {
        Some("now-str") => Ok(ftd::Value::String {
            text: std::str::from_utf8(
                std::process::Command::new("date")
                    .output()
                    .expect("failed to execute process")
                    .stdout
                    .as_slice(),
            )
            .unwrap()
            .to_string(),
            source: ftd::TextSource::Header,
        }),
        _ => ftd::ftd2021::p2::utils::e2(format!("{} not found 3", variable).as_str(), doc_name, 0),
    }
}

fn resolve_ftd_foreign_variable_2022(
    variable: &str,
    doc_name: &str,
) -> ftd::ftd2021::p1::Result<ftd::interpreter::Value> {
    match variable.strip_prefix("fastn/time#") {
        Some("now-str") => Ok(ftd::interpreter::Value::String {
            text: std::str::from_utf8(
                std::process::Command::new("date")
                    .output()
                    .expect("failed to execute process")
                    .stdout
                    .as_slice(),
            )
            .unwrap()
            .to_string(),
        }),
        _ => ftd::ftd2021::p2::utils::e2(format!("{} not found 3", variable).as_str(), doc_name, 0),
    }
}
