use std::{
    collections::{HashMap, HashSet},
    path::Path,
    process::Command,
};

use decls_gen::vars::escape_str;

/// Helpful iterators for constructing array outputs.
/// Accepts a slice of lengths, dims, and offers the cartesian product
/// over all dims.
///
/// e.g. using dims= [1, 2, 3]
/// will offer up:
///   [0, 0, 0],
///   [0, 0, 1],
///   [0, 0, 2],
///   [0, 1, 0],
///   [0, 1, 1],
///   ...
struct CartesianProductIterator {
    dims: Vec<usize>,
    next: Vec<usize>,
}

impl CartesianProductIterator {
    pub fn new(dims: Vec<usize>) -> Option<Self> {
        if dims.is_empty() || dims.iter().any(|&d| d == 0) {
            return None;
        }

        let n = dims.len();
        Some(CartesianProductIterator {
            dims,
            next: vec![0; n],
        })
    }
}

impl Iterator for CartesianProductIterator {
    type Item = Vec<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next[0] >= self.dims[0] {
            return None;
        }

        let res = self.next.clone();

        let n = self.dims.len();
        let mut i = n - 1;
        self.next[i] += 1;
        while i > 0 && self.next[i] >= self.dims[i] {
            self.next[i] = 0;
            i -= 1;
            self.next[i] += 1;
        }

        Some(res)
    }
}

/// Delimiter printed at the end of execution, denoting the start of the
/// rest of the ATI information.
const ANALYSIS_START: &'static str = "===ATI-ANALYSIS-START===\n";

/// Delimiter used in ATI information between different sites
const SITE_DELIM: &'static str = "---\n";

/// Helper, pass in "/simple/main.rs:::ENTER" to construct:
/// /path/from/root_dir/datir/tests/simple/main.rs
pub fn prefix_with_path_from_root(site_from_tests: &str) -> String {
    let prefix = std::env::current_dir().unwrap();
    escape_str(format!("{}/tests/{site_from_tests}", prefix.display()))
}

/// Compiles `{cwd}/{test_dir}/{file_name}.rs` with the added instrumentation
/// runs it, and returns the section of the stdout stream which contains the ATI info.
pub fn compile_and_execute(path: &Path) -> String {
    let invocation_dir = std::env::current_dir().unwrap();
    let full_executable = invocation_dir.join(path);
    let source = full_executable.parent().unwrap().join("main.rs");

    // Compile command
    let compile_output = Command::new("cargo")
        .args([
            "run",
            "--",
            source.to_str().unwrap(),
            "-o",
            full_executable.to_str().unwrap(),
            "--test",
        ])
        .output()
        .unwrap();

    if !compile_output.status.success() {
        let e = String::from_utf8(compile_output.stderr).unwrap();
        panic!("Unable to compile {path:?}. Error output:\n{e}");
    }

    // Execute command
    let analysis_output = Command::new(full_executable).output().unwrap();
    if !analysis_output.status.success() {
        let e = String::from_utf8(analysis_output.stderr).unwrap();
        panic!("Unable to execute {source:?}. Error output:\n{e}");
    }

    let exec_output = String::from_utf8(analysis_output.stdout).unwrap();

    // chop off all print statements that have nothing to do with ATI
    let start = exec_output.find(ANALYSIS_START).unwrap();
    return exec_output[(start + ANALYSIS_START.len())..].into();
}

/// Checks that the ati stdout stream contains all the expected information,
/// performing a partition comparison, alongside making sure the right number
/// of sites were discovered.
pub fn verify(mut ati_stdout: &str, expected_partition: &HashMap<String, HashMap<String, usize>>) {
    // checks mappings at each site are identical to expected partition
    let mut found_sites = HashSet::new();
    while let Some(end) = ati_stdout.find(SITE_DELIM) {
        let site_info: Vec<_> = ati_stdout[..end].split("\n").collect();
        let mut site_iter = site_info.into_iter().filter(|s| !s.is_empty());
        let site_name = site_iter.next().expect("Found site with no name");
        // dbg!(&site_name);

        assert!(!found_sites.contains(site_name));
        found_sites.insert(site_name);

        // map of var -> id assigned to abstract_type, at this site.
        let mut site_ati_output = HashMap::new();
        for var_info in site_iter {
            if var_info.len() < 3 {
                eprintln!("Found var:type mapping which is malformed: {}", var_info);
                continue;
            }

            let var_split: Vec<_> = var_info.split(" -> ").collect();
            site_ati_output.insert(
                String::from(var_split[0]),
                str::parse::<usize>(var_split[1]).unwrap(),
            );
        }

        // site with name has to exist
        let expected_site = expected_partition.get(site_name);
        assert!(
            expected_site.is_some(),
            "Did not expect {site_name} to exist."
        );

        let expected_site = expected_site.unwrap();
        assert_eq!(
            expected_site.len(),
            site_ati_output.len(),
            "Expected site {site_name} has a different number of \
            registered parameter mappings ({}) than observed ({})", 
            expected_site.len(), site_ati_output.len()
        );

        let mut expected_to_actual: HashMap<&usize, &usize> = HashMap::new();
        for (var, actual_id) in site_ati_output.iter() {
            let expected_id = expected_site
                .get(var)
                .expect(&format!("Expected site {site_name} does not include observed var: {var}"));
            if let Some(prev_actual_id) = expected_to_actual.get(expected_id) {
                assert_eq!(
                    **prev_actual_id, *actual_id,
                    "Var {var} was found in a wrong set at site {site_name}"
                );
            } else {
                expected_to_actual.insert(expected_id, actual_id);
            }
        }

        ati_stdout = &ati_stdout[(end + SITE_DELIM.len())..];
    }

    // found_sites contains no duplicates, so as long as we were able
    assert!(found_sites.len() == expected_partition.len());
}

pub fn delete(exec: &Path) {
    match std::fs::remove_file(exec) {
        Ok(_) => {}
        Err(_) => println!("Unable to remove old file, skipping old output deletion."),
    }
}

pub struct ExpectedSite {
    name: String,
    partition: HashMap<String, usize>,
}

impl ExpectedSite {
    pub fn new(name: String) -> Self {
        Self {
            name,
            partition: HashMap::new(),
        }
    }

    pub fn register(mut self, var: &str, comparibility: usize) -> Self {
        self.partition.insert(String::from(var), comparibility);
        self
    }

    pub fn register_array_old(
        mut self,
        name: &str,
        mut dims: Vec<usize>,
        elem_comparibility: usize,
        dim_comparibility: Vec<usize>,
    ) -> Self {
        let name = String::from(name);
        let idx_f_string = "[-<>-]".repeat(dims.len());
        for dim_idxs in CartesianProductIterator::new(dims.clone()).unwrap() {
            let mut repr = idx_f_string.clone();
            for i in dim_idxs {
                repr = repr.replacen("-<>-", &i.to_string(), 1);
            }

            self.partition
                .insert(format!("{}{}", name, repr), elem_comparibility);
        }

        let dim_len = dims.len();
        for i in 1..dim_len {
            dims.pop();
            let len_f_string = "[-<>-]".repeat(dims.len());
            for dim_idxs in CartesianProductIterator::new(dims.clone()).unwrap() {
                let mut repr = len_f_string.clone();
                for i in dim_idxs {
                    repr = repr.replacen("-<>-", &i.to_string(), 1);
                }

                self.partition.insert(
                    format!("{}{}.length", name, repr),
                    dim_comparibility[dim_comparibility.len() - i],
                );
            }
        }

        self.partition
            .insert(format!("{}.length", name), dim_comparibility[0]);

        self
    }

    pub fn build(self) -> (String, HashMap<String, usize>) {
        (self.name, self.partition)
    }
}

#[derive(Default)]
pub struct ExpectedOutput(HashMap<String, HashMap<String, usize>>);
impl ExpectedOutput {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
    pub fn register_site(&mut self, site: ExpectedSite) {
        let (name, site) = site.build();
        self.0.insert(name, site);
    }

    pub fn inner(&self) -> &HashMap<String, HashMap<String, usize>> {
        &self.0
    }
}

// FIXME: move away from print based tests, use actual assertions
#[cfg(test)]
mod tests {
    use crate::common::{CartesianProductIterator, ExpectedSite, prefix_with_path_from_root};

    #[test]
    fn test_iter() {
        let dims: Vec<usize> = vec![3, 4, 5];
        let iter = CartesianProductIterator::new(dims).unwrap();
        for x in iter {
            println!("{x:?}");
        }
    }

    #[test]
    fn test_array_site() {
        let site = ExpectedSite::new(prefix_with_path_from_root("common/main.rs::test::site"))
            .register_array_old("arr", vec![3, 4, 5], 0, vec![1, 2, 3]);

        println!("{:#?}", site.build());
    }
}
