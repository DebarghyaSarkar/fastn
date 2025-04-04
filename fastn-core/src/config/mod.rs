// Document: https://fastn_core.dev/crate/config/
// Document: https://fastn_core.dev/crate/package/

pub(crate) mod utils;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum FTDEdition {
    FTD2021,
    #[default]
    FTD2022,
    FTD2023,
}

impl FTDEdition {
    pub(crate) fn from_string(s: &str) -> fastn_core::Result<FTDEdition> {
        match s {
            "2022" => Ok(FTDEdition::FTD2022),
            "2023" => Ok(FTDEdition::FTD2023),
            t => {
                fastn_core::usage_error(format!("Unknown edition `{}`. Help use `2022` instead", t))
            }
        }
    }
    pub(crate) fn is_2023(&self) -> bool {
        matches!(self, fastn_core::FTDEdition::FTD2023)
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    // Global Information
    pub package: fastn_core::Package,
    pub root: camino::Utf8PathBuf,
    pub packages_root: camino::Utf8PathBuf,
    pub original_directory: camino::Utf8PathBuf,
    pub all_packages: std::cell::RefCell<std::collections::BTreeMap<String, fastn_core::Package>>,
    pub downloaded_assets: std::collections::BTreeMap<String, String>,
    pub global_ids: std::collections::HashMap<String, String>,
    pub named_parameters: Vec<(String, ftd::Value)>,
    pub extra_data: std::collections::BTreeMap<String, String>,
    pub current_document: Option<String>,
    pub dependencies_during_render: Vec<String>,
    pub request: Option<fastn_core::http::Request>, // TODO: It should only contain reference
    pub ftd_edition: FTDEdition,
    pub ftd_external_js: Vec<String>,
    pub ftd_inline_js: Vec<String>,
    pub ftd_external_css: Vec<String>,
    pub ftd_inline_css: Vec<String>,
}

impl Config {
    /// `build_dir` is where the static built files are stored. `fastn build` command creates this
    /// folder and stores its output here.
    pub fn build_dir(&self) -> camino::Utf8PathBuf {
        self.root.join(".build")
    }

    pub fn clone_dir(&self) -> camino::Utf8PathBuf {
        self.root.join(".clone-state")
    }

    pub fn workspace_file(&self) -> camino::Utf8PathBuf {
        self.clone_dir().join("workspace.ftd")
    }

    pub fn clone_available_crs_path(&self) -> camino::Utf8PathBuf {
        self.clone_dir().join("cr")
    }

    pub fn cr_path(&self, cr_number: usize) -> camino::Utf8PathBuf {
        self.root.join(fastn_core::cr::cr_path(cr_number))
    }

    pub fn path_without_root(&self, path: &camino::Utf8PathBuf) -> fastn_core::Result<String> {
        Ok(path.strip_prefix(&self.root)?.to_string())
    }

    pub fn cr_deleted_file_path(&self, cr_number: usize) -> camino::Utf8PathBuf {
        self.cr_path(cr_number).join("-/deleted.ftd")
    }

    pub fn track_path(&self, path: &camino::Utf8PathBuf) -> camino::Utf8PathBuf {
        let path_without_root = self
            .path_without_root(path)
            .unwrap_or_else(|_| path.to_string());
        let track_path = format!("{}.track", path_without_root);
        self.track_dir().join(track_path)
    }

    pub fn cr_track_dir(&self, cr_number: usize) -> camino::Utf8PathBuf {
        self.track_dir().join(fastn_core::cr::cr_path(cr_number))
    }

    pub fn cr_track_path(
        &self,
        path: &camino::Utf8PathBuf,
        cr_number: usize,
    ) -> camino::Utf8PathBuf {
        let path_without_root = self
            .cr_path(cr_number)
            .join(path)
            .to_string()
            .replace(self.root.to_string().as_str(), "");
        let track_path = format!("{}.track", path_without_root);
        self.track_dir().join(track_path)
    }

    pub fn cr_about_path(&self, cr_number: usize) -> camino::Utf8PathBuf {
        self.cr_path(cr_number).join("-/about.ftd")
    }

    pub fn cr_meta_path(&self, cr_number: usize) -> camino::Utf8PathBuf {
        self.cr_path(cr_number).join("-/meta.ftd")
    }

    pub(crate) fn package_info_package(&self) -> &str {
        match self
            .package
            .get_dependency_for_interface(fastn_core::FASTN_UI_INTERFACE)
            .or_else(|| {
                self.package
                    .get_dependency_for_interface(fastn_core::PACKAGE_THEME_INTERFACE)
            }) {
            Some(dep) => dep.package.name.as_str(),
            None => fastn_core::FASTN_UI_INTERFACE,
        }
    }

    pub fn remote_dir(&self) -> camino::Utf8PathBuf {
        self.root.join(".remote-state")
    }

    pub fn remote_history_dir(&self) -> camino::Utf8PathBuf {
        self.remote_dir().join("history")
    }

    /// location that stores lowest available cr number
    pub fn remote_cr(&self) -> camino::Utf8PathBuf {
        self.remote_dir().join("cr")
    }

    pub fn history_file(&self) -> camino::Utf8PathBuf {
        self.remote_dir().join("history.ftd")
    }

    pub(crate) fn history_path(&self, id: &str, version: i32) -> camino::Utf8PathBuf {
        let id_with_timestamp_extension = fastn_core::utils::snapshot_id(id, &(version as u128));
        self.remote_history_dir().join(id_with_timestamp_extension)
    }

    /// document_name_with_default("index.ftd") -> /
    /// document_name_with_default("foo/index.ftd") -> /foo/
    /// document_name_with_default("foo/abc") -> /foo/abc/
    /// document_name_with_default("/foo/abc.ftd") -> /foo/abc/
    pub(crate) fn document_name_with_default(&self, document_path: &str) -> String {
        let name = self
            .doc_id()
            .unwrap_or_else(|| document_path.to_string())
            .trim_matches('/')
            .to_string();
        if name.is_empty() {
            "/".to_string()
        } else {
            format!("/{}/", name)
        }
    }

    /// history of a fastn package is stored in `.history` folder.
    ///
    /// Current design is wrong, we should move this helper to `fastn_core::Package` maybe.
    ///
    /// History of a package is considered part of the package, and when a package is downloaded we
    /// have to chose if we want to download its history as well. For now we do not. Eventually in
    /// we will be able to say download the history also for some package.
    ///
    /// ```ftd
    /// -- fastn.dependency: django
    ///  with-history: true
    /// ```
    ///     
    /// `.history` file is created or updated by `fastn sync` command only, no one else should edit
    /// anything in it.
    pub fn history_dir(&self) -> camino::Utf8PathBuf {
        self.root.join(".history")
    }

    pub fn fastn_dir(&self) -> camino::Utf8PathBuf {
        self.root.join(".fastn")
    }

    pub fn conflicted_dir(&self) -> camino::Utf8PathBuf {
        self.fastn_dir().join("conflicted")
    }

    /// every package's `.history` contains a file `.latest.ftd`. It looks a bit link this:
    ///
    /// ```ftd
    /// -- import: fastn
    ///
    /// -- fastn.snapshot: FASTN.ftd
    /// timestamp: 1638706756293421000
    ///
    /// -- fastn.snapshot: blog.ftd
    /// timestamp: 1638706756293421000
    /// ```
    ///
    /// One `fastn.snapshot` for every file that is currently part of the package.
    pub fn latest_ftd(&self) -> camino::Utf8PathBuf {
        self.root.join(".history/.latest.ftd")
    }

    /// track_dir returns the directory where track files are stored. Tracking information as well
    /// is considered part of a package, but it is not downloaded when a package is downloaded as
    /// a dependency of another package.
    pub fn track_dir(&self) -> camino::Utf8PathBuf {
        self.root.join(".tracks")
    }

    /// `is_translation_package()` is a helper to tell you if the current package is a translation
    /// of another package. We may delete this helper soon.
    pub fn is_translation_package(&self) -> bool {
        self.package.translation_of.is_some()
    }

    /// original_path() returns the path of the original package if the current package is a
    /// translation package. it returns the path in `.packages` folder where the
    pub fn original_path(&self) -> fastn_core::Result<camino::Utf8PathBuf> {
        let o = match self.package.translation_of.as_ref() {
            Some(ref o) => o,
            None => {
                return Err(fastn_core::Error::UsageError {
                    message: "This package is not a translation package".to_string(),
                });
            }
        };
        match &o.fastn_path {
            Some(fastn_path) => Ok(fastn_path
                .parent()
                .expect("Expect fastn_path parent. Panic!")
                .to_owned()),
            _ => Err(fastn_core::Error::UsageError {
                message: format!("Unable to find `fastn_path` of the package {}", o.name),
            }),
        }
    }

    /*/// aliases() returns the list of the available aliases at the package level.
    pub fn aliases(&self) -> fastn_core::Result<std::collections::BTreeMap<&str, &fastn_core::Package>> {
        let mut resp = std::collections::BTreeMap::new();
        self.package
            .dependencies
            .iter()
            .filter(|d| d.alias.is_some())
            .for_each(|d| {
                resp.insert(d.alias.as_ref().unwrap().as_str(), &d.package);
            });
        Ok(resp)
    }*/

    /// `get_font_style()` returns the HTML style tag which includes all the fonts used by any
    /// ftd document. Currently this function does not check for fonts in package dependencies
    /// nor it tries to avoid fonts that are configured but not needed in current document.
    pub fn get_font_style(&self) -> String {
        use itertools::Itertools;
        // TODO: accept list of actual fonts used in the current document. each document accepts
        //       a different list of fonts and only fonts used by a given document should be
        //       included in the HTML produced by that font
        // TODO: fetch fonts from package dependencies as well (ideally this function should fail
        //       if one of the fonts used by any ftd document is not found

        let generated_style = {
            let mut generated_style = self
                .package
                .get_flattened_dependencies()
                .into_iter()
                .unique_by(|dep| dep.package.name.clone())
                .collect_vec()
                .iter()
                .fold(self.package.get_font_html(), |accumulator, dep| {
                    format!(
                        "{pre}\n{new}",
                        pre = accumulator,
                        new = dep.package.get_font_html()
                    )
                });
            generated_style = self.all_packages.borrow().values().fold(
                generated_style,
                |accumulator, package| {
                    format!(
                        "{pre}\n{new}",
                        pre = accumulator,
                        new = package.get_font_html()
                    )
                },
            );
            generated_style
        };
        return match generated_style.trim().is_empty() {
            false => format!("<style>{}</style>", generated_style),
            _ => "".to_string(),
        };
    }

    pub(crate) async fn download_fonts(&self) -> fastn_core::Result<()> {
        use itertools::Itertools;

        let mut fonts = vec![];
        for dep in self
            .package
            .get_flattened_dependencies()
            .into_iter()
            .unique_by(|dep| dep.package.name.clone())
        {
            fonts.extend(dep.package.fonts);
        }

        for package in self.all_packages.borrow().values() {
            fonts.extend(package.fonts.clone());
        }

        for font in fonts.iter() {
            if let Some(url) = font.get_url() {
                if fastn_core::config::utils::is_http_url(&url) {
                    continue;
                }
                let start = std::time::Instant::now();
                print!("Processing {} ... ", url);
                let content = self.get_file_and_resolve(url.as_str()).await?.1;
                fastn_core::utils::update(&self.build_dir().join(&url), content.as_slice()).await?;
                fastn_core::utils::print_end(format!("Processed {}", url).as_str(), start);
            }
        }

        Ok(())
    }

    /// update the config.global_ids map from the contents of a file
    /// in case the user defines the id for any component in the document
    pub async fn update_global_ids_from_file(
        &mut self,
        doc_id: &str,
        data: &str,
    ) -> fastn_core::Result<()> {
        /// updates the config.global_ids map
        ///
        /// mapping from [id -> link]
        ///
        /// link: <document-id>#<slugified-id>
        fn update_id_map(
            global_ids: &mut std::collections::HashMap<String, String>,
            id_string: &str,
            doc_name: &str,
            line_number: usize,
        ) -> fastn_core::Result<()> {
            // returns doc-id from link as String
            fn fetch_doc_id_from_link(link: &str) -> fastn_core::Result<String> {
                // link = <document-id>#<slugified-id>
                let doc_id = link.split_once('#').map(|s| s.0);
                match doc_id {
                    Some(id) => Ok(id.to_string()),
                    None => Err(fastn_core::Error::PackageError {
                        message: format!("Invalid link format {}", link),
                    }),
                }
            }

            let (_header, value) =
                ftd::ftd2021::p2::utils::split_once(id_string, doc_name, line_number)?;
            let document_id = fastn_core::library::convert_to_document_id(doc_name);

            if let Some(id) = value {
                // check if the current id already exists in the map
                // if it exists then throw error
                if global_ids.contains_key(&id) {
                    return Err(fastn_core::Error::UsageError {
                        message: format!(
                            "conflicting id: \'{}\' used in doc: \'{}\' and doc: \'{}\'",
                            id,
                            fetch_doc_id_from_link(&global_ids[&id])?,
                            document_id
                        ),
                    });
                }

                // mapping id -> <document-id>#<slugified-id>
                let link = format!("{}#{}", document_id, slug::slugify(&id));
                global_ids.insert(id, link);
            }

            Ok(())
        }

        // Vec<captured_id, line_number>
        let captured_global_ids: Vec<(String, usize)> =
            ftd::ftd2021::p1::parse_file_for_global_ids(data);
        for (captured_id, ln) in captured_global_ids.iter() {
            update_id_map(&mut self.global_ids, captured_id.as_str(), doc_id, *ln)?;
        }

        Ok(())
    }

    pub(crate) async fn get_versions(
        &self,
        package: &fastn_core::Package,
    ) -> fastn_core::Result<std::collections::HashMap<fastn_core::Version, Vec<fastn_core::File>>>
    {
        let path = self.get_root_for_package(package);
        let mut hash: std::collections::HashMap<fastn_core::Version, Vec<fastn_core::File>> =
            std::collections::HashMap::new();

        let all_files = self.get_all_file_paths1(package, true)?;

        for file in all_files {
            if file.is_dir() {
                continue;
            }
            let version = get_version(&file, &path).await?;
            let file = fastn_core::get_file(
                package.name.to_string(),
                &file,
                &(if version.original.eq("BASE_VERSION") {
                    path.to_owned()
                } else {
                    path.join(&version.original)
                }),
            )
            .await?;
            if let Some(files) = hash.get_mut(&version) {
                files.push(file)
            } else {
                hash.insert(version, vec![file]);
            }
        }
        return Ok(hash);

        async fn get_version(
            x: &camino::Utf8PathBuf,
            path: &camino::Utf8PathBuf,
        ) -> fastn_core::Result<fastn_core::Version> {
            let id = match tokio::fs::canonicalize(x)
                .await?
                .to_str()
                .unwrap()
                .rsplit_once(
                    if path.as_str().ends_with(std::path::MAIN_SEPARATOR) {
                        path.as_str().to_string()
                    } else {
                        format!("{}{}", path, std::path::MAIN_SEPARATOR)
                    }
                    .as_str(),
                ) {
                Some((_, id)) => id.to_string(),
                None => {
                    return Err(fastn_core::Error::UsageError {
                        message: format!("{:?} should be a file", x),
                    });
                }
            };
            if let Some((v, _)) = id.split_once('/') {
                fastn_core::Version::parse(v)
            } else {
                Ok(fastn_core::Version::base())
            }
        }
    }

    pub(crate) fn get_root_for_package(
        &self,
        package: &fastn_core::Package,
    ) -> camino::Utf8PathBuf {
        if let Some(package_fastn_path) = &package.fastn_path {
            // TODO: Unwrap?
            package_fastn_path.parent().unwrap().to_owned()
        } else if package.name.eq(&self.package.name) {
            self.root.clone()
        } else {
            self.packages_root.clone().join(package.name.as_str())
        }
    }

    pub(crate) async fn get_files(
        &self,
        package: &fastn_core::Package,
    ) -> fastn_core::Result<Vec<fastn_core::File>> {
        let path = self.get_root_for_package(package);
        let all_files = self.get_all_file_paths1(package, true)?;
        // TODO: Unwrap?
        let mut documents =
            fastn_core::paths_to_files(package.name.as_str(), all_files, &path).await?;
        documents.sort_by_key(|v| v.get_id().to_string()); // TODO: why is to_string() needed?

        Ok(documents)
    }

    /// updates the terms map from the files of the current package
    async fn update_ids_from_package(&mut self) -> fastn_core::Result<()> {
        let path = self.get_root_for_package(&self.package);
        let all_files_path = self.get_all_file_paths1(&self.package, true)?;

        let documents =
            fastn_core::paths_to_files(self.package.name.as_str(), all_files_path, &path).await?;
        for document in documents.iter() {
            if let fastn_core::File::Ftd(doc) = document {
                // Ignore fetching id's from FASTN.ftd since
                // id's would be used to link inside sitemap
                if doc.id.eq("FASTN.ftd") {
                    continue;
                }
                self.update_global_ids_from_file(&doc.id, &doc.content)
                    .await?;
            }
        }
        Ok(())
    }

    pub(crate) fn get_all_file_paths1(
        &self,
        package: &fastn_core::Package,
        ignore_history: bool,
    ) -> fastn_core::Result<Vec<camino::Utf8PathBuf>> {
        let path = self.get_root_for_package(package);
        let mut ignore_paths = ignore::WalkBuilder::new(&path);
        // ignore_paths.hidden(false); // Allow the linux hidden files to be evaluated
        ignore_paths.overrides(fastn_core::file::package_ignores(
            package,
            &path,
            ignore_history,
        )?);
        Ok(ignore_paths
            .build()
            .flatten()
            .map(|x| camino::Utf8PathBuf::from_path_buf(x.into_path()).unwrap()) //todo: improve error message
            .collect::<Vec<camino::Utf8PathBuf>>())
    }

    pub(crate) fn get_all_file_path(
        &self,
        package: &fastn_core::Package,
        ignore_paths: Vec<String>,
    ) -> fastn_core::Result<Vec<camino::Utf8PathBuf>> {
        let path = self.get_root_for_package(package);
        let mut ignore_paths_build = ignore::WalkBuilder::new(&path);
        ignore_paths_build.hidden(false);
        ignore_paths_build.overrides(fastn_core::file::ignore_path(package, &path, ignore_paths)?);
        Ok(ignore_paths_build
            .build()
            .flatten()
            .map(|x| camino::Utf8PathBuf::from_path_buf(x.into_path()).unwrap()) //todo: improve error message
            .collect::<Vec<camino::Utf8PathBuf>>())
    }

    pub async fn get_file_by_id(
        &self,
        id: &str,
        package: &fastn_core::Package,
    ) -> fastn_core::Result<fastn_core::File> {
        let file_name = fastn_core::Config::get_file_name(&self.root, id)?;
        self.get_files(package)
            .await?
            .into_iter()
            .find(|v| v.get_id().eq(file_name.as_str()))
            .ok_or_else(|| fastn_core::Error::UsageError {
                message: format!("No such file found: {}", id),
            })
    }

    pub(crate) async fn get_file_and_package_by_cr_id(
        &mut self,
        id: &str,
        cr_number: usize,
    ) -> fastn_core::Result<fastn_core::File> {
        let file_name = self.get_cr_file_and_resolve(id, cr_number).await?.0;
        let id_without_cr_prefix = fastn_core::cr::get_id_from_cr_id(id, cr_number)?;
        let package = self
            .find_package_by_id(id_without_cr_prefix.as_str())
            .await?
            .1;

        let mut file = fastn_core::get_file(
            package.name.to_string(),
            &self.root.join(file_name),
            &self.get_root_for_package(&package),
        )
        .await?;

        if id_without_cr_prefix.contains("-/") && !id_without_cr_prefix.contains("-/about") {
            let url = id_without_cr_prefix
                .trim_end_matches("/index.html")
                .trim_matches('/');
            let extension = if matches!(file, fastn_core::File::Markdown(_)) {
                "/index.md".to_string()
            } else if matches!(file, fastn_core::File::Ftd(_)) {
                "/index.ftd".to_string()
            } else {
                "".to_string()
            };
            file.set_id(format!("{}{}", url, extension).as_str());
        }
        Ok(file)
    }

    // Input
    // path: /todos/add-todo/
    // mount-point: /todos/
    // Output
    // -/<todos-package-name>/add-todo/, <todos-package-name>, /add-todo/
    // #[tracing::instrument(skip_all)]
    pub fn get_mountpoint_sanitized_path<'a>(
        &'a self,
        package: &'a fastn_core::Package,
        path: &'a str,
    ) -> Option<(
        String,
        &'a fastn_core::Package,
        String,
        Option<&fastn_core::package::app::App>,
    )> {
        // Problem for recursive dependency is that only current package contains dependency,
        // dependent package does not contain dependency

        // For similar package
        // tracing::info!(package = package.name, path = path);
        if path.starts_with(format!("-/{}", package.name.trim_matches('/')).as_str()) {
            let path_without_package_name =
                path.trim_start_matches(format!("-/{}", package.name.trim_matches('/')).as_str());
            return Some((
                path.to_string(),
                package,
                path_without_package_name.to_string(),
                None,
            ));
        }

        for (mp, dep, app) in package.apps.iter().map(|x| (&x.mount_point, &x.package, x)) {
            if path.starts_with(mp.trim_matches('/')) {
                // TODO: Need to handle for recursive dependencies mount-point
                // Note: Currently not working because dependency of package does not contain dependencies
                let package_name = dep.name.trim_matches('/');
                let sanitized_path = path.trim_start_matches(mp.trim_start_matches('/'));
                return Some((
                    format!("-/{package_name}/{sanitized_path}"),
                    dep,
                    sanitized_path.to_string(),
                    Some(app),
                ));
            } else if path.starts_with(format!("-/{}", dep.name.trim_matches('/')).as_str()) {
                let path_without_package_name =
                    path.trim_start_matches(format!("-/{}", dep.name.trim_matches('/')).as_str());
                return Some((
                    path.to_string(),
                    dep,
                    path_without_package_name.to_string(),
                    Some(app),
                ));
            }
        }
        None
    }

    pub async fn update_sitemap(
        &self,
        package: &fastn_core::Package,
    ) -> fastn_core::Result<fastn_core::Package> {
        let fastn_path = &self.packages_root.join(&package.name).join("FASTN.ftd");

        if !fastn_path.exists() {
            let package = self.resolve_package(package).await?;
            self.add_package(&package);
        }

        let fastn_doc = utils::fastn_doc(fastn_path).await?;

        let mut package = package.clone();

        package.sitemap_temp = fastn_doc.get("fastn#sitemap")?;
        package.dynamic_urls_temp = fastn_doc.get("fastn#dynamic-urls")?;

        package.sitemap = match package.sitemap_temp.as_ref() {
            Some(sitemap_temp) => {
                let mut s = fastn_core::sitemap::Sitemap::parse(
                    sitemap_temp.body.as_str(),
                    &package,
                    self,
                    false,
                )
                .await?;
                s.readers = sitemap_temp.readers.clone();
                s.writers = sitemap_temp.writers.clone();
                Some(s)
            }
            None => None,
        };

        // Handling of `-- fastn.dynamic-urls:`
        package.dynamic_urls = {
            match &package.dynamic_urls_temp {
                Some(urls_temp) => Some(fastn_core::sitemap::DynamicUrls::parse(
                    &self.global_ids,
                    &package.name,
                    urls_temp.body.as_str(),
                )?),
                None => None,
            }
        };
        Ok(package)
    }

    // -/kameri-app.herokuapp.com/
    // .packages/kameri-app.heroku.com/index.ftd
    #[tracing::instrument(skip_all)]
    pub async fn get_file_and_package_by_id(
        &mut self,
        path: &str,
    ) -> fastn_core::Result<fastn_core::File> {
        tracing::info!(path = path);
        // This function will return file and package by given path
        // path can be mounted(mount-point) with other dependencies
        //
        // Sanitize the mountpoint request.
        // Get the package and sanitized path
        let package1;

        // TODO: The shitty code written by me ever
        let (path_with_package_name, document, path_params, extra_data) =
            if !fastn_core::file::is_static(path)? {
                let (path_with_package_name, sanitized_package, sanitized_path) =
                    match self.get_mountpoint_sanitized_path(&self.package, path) {
                        Some((new_path, package, remaining_path, _)) => {
                            // Update the sitemap of the package, if it does not contain the sitemap information
                            if package.name != self.package.name {
                                package1 = self.update_sitemap(package).await?;
                                (new_path, &package1, remaining_path)
                            } else {
                                (new_path, package, remaining_path)
                            }
                        }
                        None => (path.to_string(), &self.package, path.to_string()),
                    };

                // Getting `document` with dynamic parameters, if exists
                // It will first resolve in sitemap
                // Then it will resolve in the dynamic urls
                let (document, path_params, extra_data) =
                    fastn_core::sitemap::resolve(sanitized_package, &sanitized_path)?;

                // document with package-name prefix
                let document = document.map(|doc| {
                    format!(
                        "-/{}/{}",
                        sanitized_package.name.trim_matches('/'),
                        doc.trim_matches('/')
                    )
                });
                (path_with_package_name, document, path_params, extra_data)
            } else {
                (path.to_string(), None, vec![], Default::default())
            };

        let path = path_with_package_name.as_str();

        if let Some(id) = document {
            let file_name = self.get_file_path_and_resolve(id.as_str()).await?;
            let package = self.find_package_by_id(id.as_str()).await?.1;
            let file = fastn_core::get_file(
                package.name.to_string(),
                &self.root.join(file_name),
                &self.get_root_for_package(&package),
            )
            .await?;
            self.current_document = Some(path.to_string());
            self.named_parameters = path_params;
            self.extra_data = extra_data;
            Ok(file)
        } else {
            // -/fifthtry.github.io/todos/add-todo/
            // -/fifthtry.github.io/doc-site/add-todo/
            let file_name = self.get_file_path_and_resolve(path).await?;
            // .packages/todos/add-todo.ftd
            // .packages/fifthtry.github.io/doc-site/add-todo.ftd

            let package = self.find_package_by_id(path).await?.1;
            let mut file = fastn_core::get_file(
                package.name.to_string(),
                &self.root.join(file_name.trim_start_matches('/')),
                &self.get_root_for_package(&package),
            )
            .await?;

            if path.contains("-/") {
                let url = path.trim_end_matches("/index.html").trim_matches('/');
                let extension = if matches!(file, fastn_core::File::Markdown(_)) {
                    "/index.md".to_string()
                } else if matches!(file, fastn_core::File::Ftd(_)) {
                    "/index.ftd".to_string()
                } else {
                    "".to_string()
                };
                file.set_id(format!("{}{}", url, extension).as_str());
            }
            self.current_document = Some(file.get_id().to_string());
            Ok(file)
        }
    }

    pub fn doc_id(&self) -> Option<String> {
        self.current_document
            .clone()
            .map(|v| fastn_core::utils::id_to_path(v.as_str()))
            .map(|v| v.trim().replace(std::path::MAIN_SEPARATOR, "/"))
    }

    pub async fn get_file_path(&self, id: &str) -> fastn_core::Result<String> {
        let (package_name, package) = self.find_package_by_id(id).await?;
        let mut id = id.to_string();
        let mut add_packages = "".to_string();
        if let Some(new_id) = id.strip_prefix("-/") {
            // Check if the id is alias for index.ftd. eg: `/-/bar/`
            if new_id.starts_with(&package_name) || !package.name.eq(self.package.name.as_str()) {
                id = new_id.to_string();
            }
            if !package.name.eq(self.package.name.as_str()) {
                add_packages = format!(".packages/{}/", package.name);
            }
        }
        let id = {
            let mut id = id
                .split_once("-/")
                .map(|(id, _)| id)
                .unwrap_or_else(|| id.as_str())
                .trim()
                .trim_start_matches(package_name.as_str());
            if id.is_empty() {
                id = "/";
            }
            id
        };

        Ok(format!(
            "{}{}",
            add_packages,
            package
                .resolve_by_id(id, None, self.package.name.as_str())
                .await?
                .0
        ))
    }

    pub(crate) async fn get_file_path_and_resolve(&self, id: &str) -> fastn_core::Result<String> {
        Ok(self.get_file_and_resolve(id).await?.0)
    }

    pub(crate) async fn get_file_and_resolve(
        &self,
        id: &str,
    ) -> fastn_core::Result<(String, Vec<u8>)> {
        let (package_name, package) = self.find_package_by_id(id).await?;

        let package = self.resolve_package(&package).await?;
        self.add_package(&package);
        let mut id = id.to_string();
        let mut add_packages = "".to_string();
        if let Some(new_id) = id.strip_prefix("-/") {
            // Check if the id is alias for index.ftd. eg: `/-/bar/`
            if new_id.starts_with(&package_name) || !package.name.eq(self.package.name.as_str()) {
                id = new_id.to_string();
            }
            if !package.name.eq(self.package.name.as_str()) {
                add_packages = format!(".packages/{}/", package.name);
            }
        }
        let id = {
            let mut id = id
                .split_once("-/")
                .map(|(id, _)| id)
                .unwrap_or_else(|| id.as_str())
                .trim()
                .trim_start_matches(package_name.as_str());
            if id.is_empty() {
                id = "/";
            }
            id
        };

        let (file_name, content) = package
            .resolve_by_id(id, None, self.package.name.as_str())
            .await?;
        Ok((format!("{}{}", add_packages, file_name), content))
    }

    pub(crate) async fn get_cr_file_and_resolve(
        &self,
        cr_id: &str,
        cr_number: usize,
    ) -> fastn_core::Result<(String, Vec<u8>)> {
        let id_without_cr_prefix = fastn_core::cr::get_id_from_cr_id(cr_id, cr_number)?;
        let (package_name, package) = self
            .find_package_by_id(id_without_cr_prefix.as_str())
            .await?;
        let package = self.resolve_package(&package).await?;
        self.add_package(&package);
        let mut new_id = id_without_cr_prefix.to_string();
        let mut add_packages = "".to_string();
        if let Some(id) = new_id.strip_prefix("-/") {
            // Check if the id is alias for index.ftd. eg: `/-/bar/`
            if id.starts_with(&package_name) || !package.name.eq(self.package.name.as_str()) {
                new_id = id.to_string();
            }
            if !package.name.eq(self.package.name.as_str()) {
                add_packages = format!(".packages/{}/", package.name);
            }
        }
        let id = {
            let mut id = match new_id.split_once("-/") {
                Some((p1, p2))
                    if !(package_name.eq(self.package.name.as_str())
                        && fastn_core::utils::ids_matches(p2, "about")) =>
                // full id in case of about page as it's a special page
                {
                    p1.to_string()
                }
                _ => new_id,
            }
            .trim()
            .trim_start_matches(package_name.as_str())
            .to_string();
            if id.is_empty() {
                id = "/".to_string();
            }
            id
        };

        if package.name.eq(self.package.name.as_str()) {
            let file_info_map = fastn_core::cr::cr_clone_file_info(self, cr_number).await?;
            let file_info = fastn_core::package::package_doc::file_id_to_names(id.as_str())
                .into_iter()
                .find_map(|id| file_info_map.get(&id))
                .ok_or_else(|| fastn_core::Error::UsageError {
                    message: format!("{} is not found", cr_id),
                })?;

            return Ok((
                format!("{}{}", add_packages, file_info.path),
                file_info.content.to_owned(),
            ));
        }

        let (file_name, content) = package
            .resolve_by_id(id.as_str(), None, self.package.name.as_str())
            .await?;

        Ok((format!("{}{}", add_packages, file_name), content))
    }

    /// Return (package name or alias, package)
    pub(crate) async fn find_package_by_id(
        &self,
        id: &str,
    ) -> fastn_core::Result<(String, fastn_core::Package)> {
        let sanitized_id = self
            .get_mountpoint_sanitized_path(&self.package, id)
            .map(|(x, _, _, _)| x)
            .unwrap_or_else(|| id.to_string());

        let id = sanitized_id.as_str();
        let id = if let Some(id) = id.strip_prefix("-/") {
            id
        } else {
            return Ok((self.package.name.to_string(), self.package.to_owned()));
        };

        if id.starts_with(self.package.name.as_str()) {
            return Ok((self.package.name.to_string(), self.package.to_owned()));
        }

        if let Some(package) = self.package.aliases().iter().rev().find_map(|(alias, d)| {
            if id.starts_with(alias) {
                Some((alias.to_string(), (*d).to_owned()))
            } else {
                None
            }
        }) {
            return Ok(package);
        }

        for (package_name, package) in self.all_packages.borrow().iter().rev() {
            if id.starts_with(package_name) {
                return Ok((package_name.to_string(), package.to_owned()));
            }
        }

        if let Some(package_root) =
            utils::find_root_for_file(&self.packages_root.join(id), "FASTN.ftd")
        {
            let mut package = fastn_core::Package::new("unknown-package");
            package.resolve(&package_root.join("FASTN.ftd")).await?;
            self.add_package(&package);
            return Ok((package.name.to_string(), package));
        }

        Ok((self.package.name.to_string(), self.package.to_owned()))
    }

    pub(crate) async fn download_required_file(
        root: &camino::Utf8PathBuf,
        id: &str,
        package: &fastn_core::Package,
    ) -> fastn_core::Result<String> {
        use tokio::io::AsyncWriteExt;

        let id = id.trim_start_matches(package.name.as_str());

        let base =
            package
                .download_base_url
                .clone()
                .ok_or_else(|| fastn_core::Error::PackageError {
                    message: "package base not found".to_string(),
                })?;

        if id.eq("/") {
            if let Ok(string) = crate::http::http_get_str(
                format!("{}/index.ftd", base.trim_end_matches('/')).as_str(),
            )
            .await
            {
                let base = root.join(".packages").join(package.name.as_str());
                tokio::fs::create_dir_all(&base).await?;
                tokio::fs::File::create(base.join("index.ftd"))
                    .await?
                    .write_all(string.as_bytes())
                    .await?;
                return Ok(format!(".packages/{}/index.ftd", package.name));
            }
            if let Ok(string) = crate::http::http_get_str(
                format!("{}/README.md", base.trim_end_matches('/')).as_str(),
            )
            .await
            {
                let base = root.join(".packages").join(package.name.as_str());
                tokio::fs::create_dir_all(&base).await?;
                tokio::fs::File::create(base.join("README.md"))
                    .await?
                    .write_all(string.as_bytes())
                    .await?;
                return Ok(format!(".packages/{}/README.md", package.name));
            }
            return Err(fastn_core::Error::UsageError {
                message: "File not found".to_string(),
            });
        }

        let id = id.trim_matches('/').to_string();
        if let Ok(string) =
            crate::http::http_get_str(format!("{}/{}.ftd", base.trim_end_matches('/'), id).as_str())
                .await
        {
            let (prefix, id) = match id.rsplit_once('/') {
                Some((prefix, id)) => (format!("/{}", prefix), id.to_string()),
                None => ("".to_string(), id),
            };
            let base = root
                .join(".packages")
                .join(format!("{}{}", package.name.as_str(), prefix));
            tokio::fs::create_dir_all(&base).await?;
            let file_path = base.join(format!("{}.ftd", id));
            tokio::fs::File::create(&file_path)
                .await?
                .write_all(string.as_bytes())
                .await?;
            return Ok(file_path.to_string());
        }
        if let Ok(string) = crate::http::http_get_str(
            format!("{}/{}/index.ftd", base.trim_end_matches('/'), id).as_str(),
        )
        .await
        {
            let base = root.join(".packages").join(package.name.as_str()).join(id);
            tokio::fs::create_dir_all(&base).await?;
            let file_path = base.join("index.ftd");
            tokio::fs::File::create(&file_path)
                .await?
                .write_all(string.as_bytes())
                .await?;
            return Ok(file_path.to_string());
        }
        if let Ok(string) =
            crate::http::http_get_str(format!("{}/{}.md", base.trim_end_matches('/'), id).as_str())
                .await
        {
            let base = root.join(".packages").join(package.name.as_str());
            tokio::fs::create_dir_all(&base).await?;
            tokio::fs::File::create(base.join(format!("{}.md", id)))
                .await?
                .write_all(string.as_bytes())
                .await?;
            return Ok(format!(".packages/{}/{}.md", package.name, id));
        }
        if let Ok(string) = crate::http::http_get_str(
            format!("{}/{}/README.md", base.trim_end_matches('/'), id).as_str(),
        )
        .await
        {
            let base = root.join(".packages").join(package.name.as_str());
            tokio::fs::create_dir_all(&base).await?;
            tokio::fs::File::create(base.join(format!("{}/README.md", id)))
                .await?
                .write_all(string.as_bytes())
                .await?;
            return Ok(format!(".packages/{}/{}/README.md", package.name, id));
        }
        Err(fastn_core::Error::UsageError {
            message: "File not found".to_string(),
        })
    }

    pub(crate) fn get_file_name(
        root: &camino::Utf8PathBuf,
        id: &str,
    ) -> fastn_core::Result<String> {
        let mut id = id.to_string();
        let mut add_packages = "".to_string();
        if let Some(new_id) = id.strip_prefix("-/") {
            id = new_id.to_string();
            add_packages = ".packages/".to_string()
        }
        let mut id = id
            .split_once("-/")
            .map(|(id, _)| id)
            .unwrap_or_else(|| id.as_str())
            .trim()
            .replace("/index.html", "/")
            .replace("index.html", "/");
        if id.eq("/") {
            if root.join(format!("{}index.ftd", add_packages)).exists() {
                return Ok(format!("{}index.ftd", add_packages));
            }
            if root.join(format!("{}README.md", add_packages)).exists() {
                return Ok(format!("{}README.md", add_packages));
            }
            return Err(fastn_core::Error::UsageError {
                message: "File not found".to_string(),
            });
        }
        id = id.trim_matches('/').to_string();
        if root.join(format!("{}{}.ftd", add_packages, id)).exists() {
            return Ok(format!("{}{}.ftd", add_packages, id));
        }
        if root
            .join(format!("{}{}/index.ftd", add_packages, id))
            .exists()
        {
            return Ok(format!("{}{}/index.ftd", add_packages, id));
        }
        if root.join(format!("{}{}.md", add_packages, id)).exists() {
            return Ok(format!("{}{}.md", add_packages, id));
        }
        if root
            .join(format!("{}{}/README.md", add_packages, id))
            .exists()
        {
            return Ok(format!("{}{}/README.md", add_packages, id));
        }
        Err(fastn_core::Error::UsageError {
            message: "File not found".to_string(),
        })
    }

    async fn get_root_path(
        directory: &camino::Utf8PathBuf,
    ) -> fastn_core::Result<camino::Utf8PathBuf> {
        if let Some(fastn_ftd_root) = utils::find_root_for_file(directory, "FASTN.ftd") {
            return Ok(fastn_ftd_root);
        }
        let fastn_manifest_path = match utils::find_root_for_file(directory, "fastn.manifest.ftd") {
            Some(fastn_manifest_path) => fastn_manifest_path,
            None => {
                return Err(fastn_core::Error::UsageError {
                    message: "FASTN.ftd or fastn.manifest.ftd not found in any parent directory"
                        .to_string(),
                });
            }
        };

        let doc = tokio::fs::read_to_string(fastn_manifest_path.join("fastn.manifest.ftd"));
        let lib = fastn_core::FastnLibrary::default();
        let fastn_manifest_processed =
            match fastn_core::doc::parse_ftd("fastn.manifest", doc.await?.as_str(), &lib) {
                Ok(fastn_manifest_processed) => fastn_manifest_processed,
                Err(e) => {
                    return Err(fastn_core::Error::PackageError {
                        message: format!("failed to parse fastn.manifest.ftd: {:?}", &e),
                    });
                }
            };

        let new_package_root = fastn_manifest_processed
            .get::<String>("fastn.manifest#package-root")?
            .as_str()
            .split('/')
            .fold(fastn_manifest_path, |accumulator, part| {
                accumulator.join(part)
            });

        if new_package_root.join("FASTN.ftd").exists() {
            Ok(new_package_root)
        } else {
            Err(fastn_core::Error::PackageError {
                message: "Can't find FASTN.ftd. The path specified in fastn.manifest.ftd doesn't contain the FASTN.ftd file".to_string(),
            })
        }
    }

    pub fn add_edition(self, edition: Option<String>) -> fastn_core::Result<Self> {
        match edition {
            Some(e) => {
                let mut config = self;
                config.ftd_edition = FTDEdition::from_string(e.as_str())?;
                Ok(config)
            }
            None => Ok(self),
        }
    }

    pub fn add_external_js(self, external_js: Vec<String>) -> Self {
        let mut config = self;
        config.ftd_external_js = external_js;
        config
    }

    pub fn add_inline_js(self, inline_js: Vec<String>) -> Self {
        let mut config = self;
        config.ftd_inline_js = inline_js;
        config
    }

    pub fn add_external_css(self, external_css: Vec<String>) -> Self {
        let mut config = self;
        config.ftd_external_css = external_css;
        config
    }

    pub fn add_inline_css(self, inline_css: Vec<String>) -> Self {
        let mut config = self;
        config.ftd_inline_css = inline_css;
        config
    }

    /// `read()` is the way to read a Config.
    #[tracing::instrument(name = "Config::read", skip_all)]
    pub async fn read(
        root: Option<String>,
        resolve_sitemap: bool,
        req: Option<&fastn_core::http::Request>,
    ) -> fastn_core::Result<fastn_core::Config> {
        let (root, original_directory) = match root {
            Some(r) => {
                let root: camino::Utf8PathBuf = tokio::fs::canonicalize(r.as_str())
                    .await?
                    .to_str()
                    .map_or_else(|| r, |r| r.to_string())
                    .into();
                (root.clone(), root)
            }
            None => {
                let original_directory: camino::Utf8PathBuf =
                    tokio::fs::canonicalize(std::env::current_dir()?)
                        .await?
                        .try_into()?;
                (
                    fastn_core::Config::get_root_path(&original_directory).await?,
                    original_directory,
                )
            }
        };
        let fastn_doc = utils::fastn_doc(&root.join("FASTN.ftd")).await?;
        let package = fastn_core::Package::from_fastn_doc(&root, &fastn_doc)?;
        let mut config = Config {
            package: package.clone(),
            packages_root: root.clone().join(".packages"),
            root,
            original_directory,
            current_document: None,
            all_packages: Default::default(),
            downloaded_assets: Default::default(),
            extra_data: Default::default(),
            global_ids: Default::default(),
            request: req.map(ToOwned::to_owned),
            named_parameters: vec![],
            ftd_edition: FTDEdition::default(),
            ftd_external_js: Default::default(),
            ftd_inline_js: Default::default(),
            ftd_external_css: Default::default(),
            ftd_inline_css: Default::default(),
            dependencies_during_render: Default::default(),
        };

        // Update global_ids map from the current package files
        config.update_ids_from_package().await?;

        // TODO: Major refactor, while parsing sitemap of a package why do we need config in it?
        config.package.sitemap = {
            let sitemap = match package.translation_of.as_ref() {
                Some(translation) => translation,
                None => &package,
            }
            .sitemap_temp
            .as_ref();

            match sitemap {
                Some(sitemap_temp) => {
                    let mut s = fastn_core::sitemap::Sitemap::parse(
                        sitemap_temp.body.as_str(),
                        &package,
                        &config,
                        resolve_sitemap,
                    )
                    .await?;
                    s.readers = sitemap_temp.readers.clone();
                    s.writers = sitemap_temp.writers.clone();
                    Some(s)
                }
                None => None,
            }
        };

        // Handling of `-- fastn.dynamic-urls:`
        config.package.dynamic_urls = {
            match &package.dynamic_urls_temp {
                Some(urls_temp) => Some(fastn_core::sitemap::DynamicUrls::parse(
                    &config.global_ids,
                    &package.name,
                    urls_temp.body.as_str(),
                )?),
                None => None,
            }
        };

        config.add_package(&package);

        // fastn installed Apps
        config.package.apps = {
            let apps_temp: Vec<fastn_core::package::app::AppTemp> = fastn_doc.get("fastn#app")?;
            let mut apps = vec![];
            for app in apps_temp.into_iter() {
                apps.push(app.into_app(&config).await?);
            }
            apps
        };

        Ok(config)
    }

    pub fn set_request(mut self, req: fastn_core::http::Request) -> Self {
        self.request = Some(req);
        self
    }

    pub(crate) async fn resolve_package(
        &self,
        package: &fastn_core::Package,
    ) -> fastn_core::Result<fastn_core::Package> {
        if self.package.name.eq(package.name.as_str()) {
            return Ok(self.package.clone());
        }

        if let Some(package) = { self.all_packages.borrow().get(package.name.as_str()) } {
            return Ok(package.clone());
        }

        let package = package
            .get_and_resolve(&self.get_root_for_package(package))
            .await?;

        self.add_package(&package);
        Ok(package)
    }

    pub(crate) fn add_package(&self, package: &fastn_core::Package) {
        self.all_packages
            .borrow_mut()
            .insert(package.name.to_string(), package.to_owned());
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn get_fastn_document(
        &self,
        package_name: &str,
    ) -> fastn_core::Result<ftd::ftd2021::p2::Document> {
        let package = fastn_core::Package::new(package_name);
        let root = self.get_root_for_package(&package);
        let package_fastn_path = root.join("FASTN.ftd");
        let doc = std::fs::read_to_string(package_fastn_path)?;
        let lib = fastn_core::FastnLibrary::default();
        Ok(fastn_core::doc::parse_ftd("fastn", doc.as_str(), &lib)?)
    }

    pub(crate) async fn get_reserved_crs(
        &self,
        number_of_crs_to_reserve: Option<usize>,
    ) -> fastn_core::Result<Vec<i32>> {
        let number_of_crs_to_reserve =
            if let Some(number_of_crs_to_reserve) = number_of_crs_to_reserve {
                number_of_crs_to_reserve
            } else {
                fastn_core::NUMBER_OF_CRS_TO_RESERVE
            };
        if !cfg!(feature = "remote") {
            return fastn_core::usage_error("Can be used by remote only".to_string());
        }
        let value = fastn_core::cache::update(
            self.remote_cr().to_string().as_str(),
            number_of_crs_to_reserve,
        )
        .await? as i32;

        Ok(Vec::from_iter(
            (value - (number_of_crs_to_reserve as i32))..value,
        ))
    }

    #[tracing::instrument(skip(req, self))]
    pub(crate) async fn can_read(
        &self,
        req: &fastn_core::http::Request,
        document_path: &str,
        with_confidential: bool, // can read should use confidential property or not
    ) -> fastn_core::Result<bool> {
        // Function Docs
        // If user can read the document based on readers, user will have read access to page
        // If user cannot read the document based on readers, and if confidential is false so user
        // can access the page, and if confidential is true user will not be able to access the
        // document

        // can_read: true, confidential: true => true (can access)
        // can_read: true, confidential: false => true (can access)
        // can_read: false, confidential: true => false (cannot access)
        // can_read: false, confidential: false => true (can access)

        use itertools::Itertools;
        let document_name = self.document_name_with_default(document_path);
        if let Some(sitemap) = &self.package.sitemap {
            // TODO: This can be buggy in case of: if groups are used directly in sitemap are foreign groups
            let (document_readers, confidential) =
                sitemap.readers(document_name.as_str(), &self.package.groups);

            // TODO: Need to check the confidential logic, if readers are not defined in the sitemap
            if document_readers.is_empty() {
                return Ok(true);
            }
            let access_identities =
                fastn_core::user_group::access_identities(self, req, &document_name, true).await?;

            let belongs_to = fastn_core::user_group::belongs_to(
                self,
                document_readers.as_slice(),
                access_identities.iter().collect_vec().as_slice(),
            )?;

            if with_confidential {
                if belongs_to {
                    return Ok(true);
                }
                return Ok(!confidential);
            }
            return Ok(belongs_to);
        }
        Ok(true)
    }

    #[tracing::instrument(skip(req, self))]
    pub(crate) async fn can_write(
        &self,
        req: &fastn_core::http::Request,
        document_path: &str,
    ) -> fastn_core::Result<bool> {
        use itertools::Itertools;
        let document_name = self.document_name_with_default(document_path);
        if let Some(sitemap) = &self.package.sitemap {
            // TODO: This can be buggy in case of: if groups are used directly in sitemap are foreign groups
            let document_writers = sitemap.writers(document_name.as_str(), &self.package.groups);
            let access_identities =
                fastn_core::user_group::access_identities(self, req, &document_name, false).await?;

            return fastn_core::user_group::belongs_to(
                self,
                document_writers.as_slice(),
                access_identities.iter().collect_vec().as_slice(),
            );
        }

        Ok(false)
    }
}
