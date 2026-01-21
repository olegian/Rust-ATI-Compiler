use std::{collections::HashMap, process::Command};

const ANALYSIS_START: &'static str = "===ATI-ANALYSIS-START===\n";
const SITE_DELIM: &'static str = "---\n";

// TODO: Paths need to not be hardcoded, and generated binaries placed in correct folders

#[test]
fn simple() {
    let mut expected = HashMap::new();
    expected.insert("main", HashMap::new());
    expected.insert("max::ENTER", HashMap::from([("x", 0), ("y", 1)]));
    expected.insert("max::EXIT", HashMap::from([("x", 0), ("y", 0), ("RET", 0)]));

    let _compile_output = Command::new("cargo")
        .args([
            "run",
            "--",
            "--target-dir",
            "/home/olegian/TRACTOR/queries/tests/simple",
            "/home/olegian/TRACTOR/queries/tests/simple/input.rs",
        ])
        .output()
        .unwrap();

    let analysis_output = Command::new("./input").output().unwrap();
    let exec_output = String::from(str::from_utf8(&analysis_output.stdout).unwrap());

    // chop off all print statements that have nothing to do with ATI
    let start = exec_output.find(ANALYSIS_START).unwrap();
    let mut output = &exec_output[(start + ANALYSIS_START.len())..];

    // checks mappings at each site are identical to expected partition
    while let Some(end) = output.find(SITE_DELIM) {
        let site: Vec<_> = output[..end].split("\n").collect();
        if site.len() <= 1 {
            // this site is poorly formatted. This isn't good. Skipping for now
            // as I'm really using this to skip main until i figure out what to do with it
            output = &output[(end + SITE_DELIM.len())..];
            continue;
        }

        let site_name = site[0];

        let mut mappings_at_site = HashMap::new();
        for map in &site[1..] {
            if map.len() < 3 {
                continue;
            }

            let var: Vec<_> = map.split(":").collect();
            mappings_at_site.insert(var[0], str::parse::<usize>(var[1]).unwrap());
        }

        // site with name has to exist
        let expected_site = expected.get(site_name);
        assert!(expected_site.is_some());

        // and have the correct amount of variables.
        // TODO: i'm not convinced this is a strong enough check
        let expected_site = expected_site.unwrap();
        assert!(expected_site.len() == mappings_at_site.len());

        // determine actual ids are correctly mapped to expected ids
        let mut observed = HashMap::new();
        for (var, actual_id) in mappings_at_site.iter() {
            if let Some(expected_id) = observed.get(actual_id) {
                // We've seen this ati id before! mapping has to be the same
                assert!(*expected_id == expected_site.get(var).unwrap());
            } else {
                // first time seeing this, does this var even exist?
                let expected_id = expected_site.get(var);
                assert!(expected_id.is_some());

                // forever associate whatever id the analysis spit out
                // with the one we expect.
                let expected_id = expected_id.unwrap();
                observed.insert(actual_id, expected_id);
            }
        }

        output = &output[(end + SITE_DELIM.len())..];
    }
}
