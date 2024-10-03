use super::registry::{Package, PackageRegistry, RootPath};

/// Search result for a package query.
///
/// Contains the matched package, its root path `bin` `base` `pkg`,
/// and a relevance score indicating how well it matches the search query.
///
/// # Fields
///
/// * `package` - Reference to the matched package
/// * `root_path` - String indicating where the package was found ("bin", "base", or "pkg")
/// * `relevance_score` - A float value indicating the match quality (higher is better)
pub struct SearchResult<'a> {
    pub package: &'a Package,
    pub root_path: String,
    pub relevance_score: u32,
}

pub trait PackageSearch {
    /// Searches for packages matching the given query string.
    ///
    /// # Arguments
    ///
    /// * `query` - The search string to match against packages
    /// * `root_path` - Optional filter to search only in a specific root path
    ///
    /// # Returns
    ///
    /// A vector of `SearchResult`s, sorted by relevance (highest first)
    fn search(&self, query: &str, root_path: Option<RootPath>) -> Vec<SearchResult>;
}

impl PackageSearch for PackageRegistry {
    fn search(&self, query: &str, root_path: Option<RootPath>) -> Vec<SearchResult> {
        let query = query.trim().to_lowercase();

        let package_iterators = match root_path {
            Some(RootPath::Bin) => vec![(&self.bin, "bin")],
            Some(RootPath::Base) => vec![(&self.base, "base")],
            Some(RootPath::Pkg) => vec![(&self.pkg, "pkg")],
            None => vec![(&self.bin, "bin"), (&self.base, "base"), (&self.pkg, "pkg")],
        };

        fn calculate_relevance(package: &Package, query: &str) -> u32 {
            if package.name.to_lowercase() == query {
                2
            } else if package.name.to_lowercase().contains(query) {
                1
            } else {
                0
            }
        }

        let mut results: Vec<SearchResult> = package_iterators
            .into_iter()
            .flat_map(|(package_map, root_path_str)| {
                package_map.iter().filter_map({
                    let value = query.clone();
                    move |(_, package)| {
                        let relevance = calculate_relevance(package, &value);
                        if relevance > 0 {
                            Some(SearchResult {
                                package,
                                root_path: root_path_str.to_string(),
                                relevance_score: relevance,
                            })
                        } else {
                            None
                        }
                    }
                })
            })
            .collect();

        results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap());

        results
    }
}
