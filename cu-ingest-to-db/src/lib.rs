pub mod database;

/// Types describing the results of evaluating a Nixpkgs-like attrset.
pub mod derivation {
    use cu_lib_nix::nix;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(untagged)]
    pub enum StringOrList {
        String(String),
        List(Vec<String>),
    }

    // TODO: implement license fields parsing.
    pub type MetaLicense = serde_json::Value;

    #[derive(Serialize, Deserialize, Debug)]
    /// Nixpkgs-flavoured metadata attrset.
    pub struct Meta {
        pub available: Option<bool>,
        pub broken: Option<bool>,
        pub insecure: Option<bool>,
        pub unfree: Option<bool>,
        pub unsupported: Option<bool>,

        #[serde(rename = "outputsToInstall")]
        pub outputs_to_install: Option<Vec<String>>,

        pub homepage: Option<StringOrList>,

        pub description: Option<String>,
        #[serde(rename = "longDescription")]
        pub long_description: Option<String>,

        #[serde(rename = "mainProgram")]
        pub main_program: Option<String>,

        pub position: Option<String>,

        pub license: Option<MetaLicense>,
        pub licenses: Option<Vec<MetaLicense>>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    /// Data and metadata about a derivation.
    pub struct Info {
        #[serde(rename = "attrPath")]
        pub attr_path: nix::AttributePath,
        #[serde(rename = "drvPath")]
        pub drv_path: String,
        pub meta: Option<Meta>,
        pub name: String,
        pub outputs: HashMap<String, String>,
        pub pname: Option<String>,
        pub version: Option<String>,
    }

    impl Info {
        /// Formats the attribute path as the joined "deep" access form for usage in Nix.
        /// i.e. `["a" "b" "c"]` → `"a"."b"."c"`
        pub fn attribute_to_nix(&self) -> String {
            self.attr_path
                .iter()
                .map(|component| serde_json::to_string(component).unwrap())
                .collect::<Vec<String>>()
                .join(".")
        }
    }
}

#[cfg(test)]
mod tests {
    mod lib {
        use crate::derivation;

        const HELLO_DERIVATION_DATA: &str = r#"
            {
              "attrPath": [
                "hello"
              ],
              "drvPath": "/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-hello-2.12.1.drv",
              "meta": {
                "available": true,
                "broken": false,
                "changelog": "https://git.savannah.gnu.org/cgit/hello.git/plain/NEWS?h=v2.12.1",
                "description": "Program that produces a familiar, friendly greeting",
                "homepage": "https://www.gnu.org/software/hello/manual/",
                "insecure": false,
                "license": {
                  "deprecated": false,
                  "free": true,
                  "fullName": "GNU General Public License v3.0 or later",
                  "redistributable": true,
                  "shortName": "gpl3Plus",
                  "spdxId": "GPL-3.0-or-later",
                  "url": "https://spdx.org/licenses/GPL-3.0-or-later.html"
                },
                "longDescription": "GNU Hello is a program that prints \"Hello, world!\" when you run it.\nIt is fully customizable.\n",
                "mainProgram": "hello",
                "maintainers": [
                  {
                    "email": "post@steffenvogel.de",
                    "github": "stv0g",
                    "githubId": 285829,
                    "keys": [
                      {
                        "fingerprint": "09BE 3BAE 8D55 D4CD 8579  285A 9675 EAC3 4897 E6E2"
                      }
                    ],
                    "matrix": "@stv0ge:matrix.org",
                    "name": "Steffen Vogel"
                  }
                ],
                "maintainersPosition": {
                  "column": 5,
                  "file": "/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-source/pkgs/by-name/he/hello/package.nix",
                  "line": 55
                },
                "name": "hello-2.12.1",
                "outputsToInstall": [
                  "out"
                ],
                "platforms": [
                  "i686-cygwin",
                  "x86_64-cygwin",
                  "x86_64-darwin",
                  "aarch64-darwin",
                  "i686-freebsd",
                  "x86_64-freebsd",
                  "aarch64-freebsd",
                  "aarch64-genode",
                  "i686-genode",
                  "x86_64-genode",
                  "x86_64-solaris",
                  "javascript-ghcjs",
                  "aarch64-linux",
                  "armv5tel-linux",
                  "armv6l-linux",
                  "armv7a-linux",
                  "armv7l-linux",
                  "i686-linux",
                  "loongarch64-linux",
                  "m68k-linux",
                  "microblaze-linux",
                  "microblazeel-linux",
                  "mips-linux",
                  "mips64-linux",
                  "mips64el-linux",
                  "mipsel-linux",
                  "powerpc64-linux",
                  "powerpc64le-linux",
                  "riscv32-linux",
                  "riscv64-linux",
                  "s390-linux",
                  "s390x-linux",
                  "x86_64-linux",
                  "mmix-mmixware",
                  "aarch64-netbsd",
                  "armv6l-netbsd",
                  "armv7a-netbsd",
                  "armv7l-netbsd",
                  "i686-netbsd",
                  "m68k-netbsd",
                  "mipsel-netbsd",
                  "powerpc-netbsd",
                  "riscv32-netbsd",
                  "riscv64-netbsd",
                  "x86_64-netbsd",
                  "aarch64_be-none",
                  "aarch64-none",
                  "arm-none",
                  "armv6l-none",
                  "avr-none",
                  "i686-none",
                  "microblaze-none",
                  "microblazeel-none",
                  "mips-none",
                  "mips64-none",
                  "msp430-none",
                  "or1k-none",
                  "m68k-none",
                  "powerpc-none",
                  "powerpcle-none",
                  "riscv32-none",
                  "riscv64-none",
                  "rx-none",
                  "s390-none",
                  "s390x-none",
                  "vc4-none",
                  "x86_64-none",
                  "i686-openbsd",
                  "x86_64-openbsd",
                  "x86_64-redox",
                  "wasm64-wasi",
                  "wasm32-wasi",
                  "aarch64-windows",
                  "x86_64-windows",
                  "i686-windows"
                ],
                "position": "/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-source/pkgs/by-name/he/hello/package.nix:47",
                "unfree": false,
                "unsupported": false
              },
              "name": "hello-2.12.1",
              "outputs": {
                "out": "/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-hello-2.12.1"
              },
              "pname": "hello",
              "version": "2.12.1"
            }
        "#;

        #[test]
        fn test_derivation_data() {
            let data: derivation::Info = serde_json::from_str(HELLO_DERIVATION_DATA).unwrap();
            assert_eq!(data.name, "hello-2.12.1".to_string());
            assert_eq!(data.pname, Some("hello".to_string()));
            assert_eq!(data.version, Some("2.12.1".to_string()));
            assert_eq!(data.attr_path, vec!("hello"));
            assert_eq!(
                data.meta.unwrap().description,
                Some("Program that produces a familiar, friendly greeting".to_string())
            );
        }
    }
}
