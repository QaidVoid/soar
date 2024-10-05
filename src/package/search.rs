use super::{
    registry::{Package, PackageRegistry, ResolvedPackage, RootPath},
    util::PackageQuery,
};

impl PackageRegistry {
    /// Searches for packages matching the given query string.
    pub fn search(&self, query: &PackageQuery) -> Vec<ResolvedPackage> {
        let pkg_name = query.name.trim().to_lowercase();

        let package_iterators = query
            .root_path
            .to_owned()
            .map(|root_path| match root_path {
                RootPath::Bin => vec![(&self.bin, RootPath::Bin)],
                RootPath::Base => vec![(&self.base, RootPath::Base)],
                RootPath::Pkg => vec![(&self.pkg, RootPath::Pkg)],
            })
            .unwrap_or_else(|| {
                vec![
                    (&self.bin, RootPath::Bin),
                    (&self.base, RootPath::Base),
                    (&self.pkg, RootPath::Pkg),
                ]
            });

        let mut pkgs: Vec<(u32, Package, RootPath)> = package_iterators
            .iter()
            .flat_map(|(map, root_path)| {
                map.iter().flat_map(|(_, packages)| {
                    packages.iter().filter_map(|pkg| {
                        let mut score = 0;
                        if pkg.name == pkg_name {
                            score += 2;
                        } else if pkg.name.contains(&pkg_name) {
                            score += 1;
                        } else {
                            return None;
                        }

                        if query.variant.is_none() || pkg.variant.as_ref() == query.variant.as_ref()
                        {
                            Some((score, pkg.to_owned(), root_path.to_owned()))
                        } else {
                            None
                        }
                    })
                })
            })
            .collect();

        pkgs.sort_by(|(a, _, _), (b, _, _)| b.cmp(a));

        let pkgs: Vec<ResolvedPackage> = pkgs
            .into_iter()
            .filter(|(score, _, _)| *score > 0)
            .collect::<Vec<_>>()
            .into_iter()
            .map(|(_, pkg, root_path)| ResolvedPackage {
                package: pkg,
                root_path,
            })
            .collect();

        pkgs
    }
}
