//! Loaded APK + VM session.

use crate::android::apk::{self, LoadedApk};
use crate::android::error::{AndroidError, Result};
use crate::android::fixture;
use crate::android::hle::Value;
use crate::android::interp::Vm;

#[derive(Debug)]
pub struct AppRuntime {
    pub apk: LoadedApk,
    pub vm: Vm,
}

impl AppRuntime {
    pub fn load(apk_bytes: &[u8]) -> Result<Self> {
        let apk = apk::load_apk_bytes(apk_bytes)?;
        let vm = Vm::new(apk.dex.clone());
        Ok(Self { apk, vm })
    }

    pub fn load_hello_fixture() -> Result<Self> {
        let bytes = fixture::build_hello_apk()?;
        Self::load(&bytes)
    }

    pub fn info_json(&self) -> String {
        let act = self
            .apk
            .launcher_activity
            .clone()
            .unwrap_or_else(|| "".into());
        format!(
            r#"{{"package":"{}","activity":"{}","apk_size":{},"classes":{},"halted":{},"steps":{},"logs":{}}}"#,
            escape(&self.apk.package_name),
            escape(&act),
            self.apk.apk_size,
            self.apk.dex.classes.len(),
            self.vm.halted,
            self.vm.steps,
            self.vm.host.logs.len()
        )
    }

    pub fn launch(&mut self) -> Result<()> {
        let class = self
            .apk
            .launcher_activity
            .clone()
            .or_else(|| {
                self.apk
                    .dex
                    .classes
                    .first()
                    .map(|c| c.descriptor.clone())
            })
            .ok_or_else(|| AndroidError::Runtime("no activity/class".into()))?;

        // Prefer main(String[]), then onCreate.
        if self.apk.dex.find_method(&class, "main").is_some() {
            let args = [Value::Obj(0)]; // null String[] is fine for our fixture
            self.vm.call_method(&class, "main", &args)?;
        } else if self.apk.dex.find_method(&class, "onCreate").is_some() {
            let this = self.vm.host.alloc_instance(class.clone());
            self.vm.call_method(&class, "onCreate", &[this])?;
        } else {
            return Err(AndroidError::Runtime(format!(
                "no main/onCreate on {class}"
            )));
        }
        Ok(())
    }

    pub fn step(&mut self) -> Result<bool> {
        self.vm.step()
    }

    pub fn run_batch(&mut self, n: u32) -> Result<u32> {
        self.vm.run_batch(n)
    }

    pub fn logs_text(&self) -> String {
        self.vm.host.logs.join("\n")
    }

    pub fn canvas_text(&self) -> String {
        self.vm.host.canvas_text.clone()
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_apk_runs_and_logs() {
        let mut rt = AppRuntime::load_hello_fixture().expect("load");
        assert!(rt.apk.package_name.contains("nekodroid") || rt.apk.package_name == "com.nekodroid");
        rt.launch().expect("launch");
        let n = rt.run_batch(10_000).expect("run");
        assert!(n > 0);
        assert!(rt.vm.halted);
        let logs = rt.logs_text();
        assert!(
            logs.contains("Hello from NekoDroid"),
            "logs were: {logs}"
        );
    }

    #[test]
    fn write_fixtures_to_testdata() {
        use crate::android::fixture;
        let dex = fixture::build_hello_dex().expect("dex");
        let apk = fixture::build_hello_apk().expect("apk");
        let _ = std::fs::create_dir_all("testdata");
        std::fs::write("testdata/hello_println.dex", &dex).expect("write dex");
        std::fs::write("testdata/hello.apk", &apk).expect("write apk");
        std::fs::write("testdata/bad.apk", b"not-an-apk").expect("write bad");
        assert!(apk.len() > 64);
        assert_eq!(&dex[0..4], b"dex\n");
    }

    #[test]
    fn bad_apk_errors() {
        let err = AppRuntime::load(b"not an apk");
        assert!(err.is_err());
    }

    #[test]
    fn dalvik_array_and_binop_helpers() {
        use crate::android::decode;
        use crate::android::hle::Value;
        use crate::android::opcodes;

        let insns = [
            0x0312u16, // const/4 v0, #3
            0x0412,    // const/4 v1, #4
            0x10b0,    // add-int/2addr v0, v1
            0x000f,    // return v0
        ];
        assert_eq!(decode::decode(&insns, 0).unwrap().op, opcodes::CONST_4);
        assert_eq!(decode::decode(&insns, 2).unwrap().op, opcodes::ADD_INT_2ADDR);
        assert_eq!(decode::decode(&insns, 2).unwrap().size, 1);

        let mut host = crate::android::hle::HleHost::default();
        let arr = host.alloc_array(2);
        assert_eq!(host.array_len(arr), Some(2));
        assert!(host.array_set(arr, 1, Value::Int(9)));
        assert_eq!(host.array_get(arr, 1), Some(Value::Int(9)));
    }
}
