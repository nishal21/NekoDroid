//! Framework HLE stubs.

use crate::android::dex::MethodId;

#[derive(Debug, Clone, Default)]
pub struct HleHost {
    pub logs: Vec<String>,
    pub canvas_text: String,
    pub objects: Vec<HleObject>,
    pub touch_x: i32,
    pub touch_y: i32,
    pub touch_down: bool,
}

#[derive(Debug, Clone)]
pub enum HleObject {
    String(String),
    Instance {
        class: String,
        fields: Vec<(u32, Value)>,
    },
    Array {
        len: i32,
        elems: Vec<Value>,
    },
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Int(i32),
    Obj(u32), // index into HleHost.objects (0 = null)
}

impl Value {
    pub fn as_int(self) -> i32 {
        match self {
            Value::Int(v) => v,
            Value::Obj(0) => 0,
            Value::Obj(i) => i as i32,
        }
    }
}

impl HleHost {
    pub fn alloc_string(&mut self, s: String) -> Value {
        self.objects.push(HleObject::String(s));
        Value::Obj(self.objects.len() as u32)
    }

    pub fn alloc_instance(&mut self, class: String) -> Value {
        self.objects.push(HleObject::Instance {
            class,
            fields: Vec::new(),
        });
        Value::Obj(self.objects.len() as u32)
    }

    /// Rejects negative lengths and lengths above 1 MiB elements.
    pub fn alloc_array(&mut self, len: i32) -> crate::android::error::Result<Value> {
        const MAX_ARRAY_LEN: i32 = 1_048_576;
        if len < 0 || len > MAX_ARRAY_LEN {
            return Err(crate::android::error::AndroidError::Vm(format!(
                "new-array length {len} rejected (cap {MAX_ARRAY_LEN})"
            )));
        }
        let n = len as usize;
        self.objects.push(HleObject::Array {
            len,
            elems: vec![Value::Int(0); n],
        });
        Ok(Value::Obj(self.objects.len() as u32))
    }

    pub fn instance_get(&self, obj: Value, field: u32) -> Option<Value> {
        let Value::Obj(i) = obj else {
            return None;
        };
        if i == 0 {
            return None;
        }
        match self.objects.get((i - 1) as usize) {
            Some(HleObject::Instance { fields, .. }) => {
                fields.iter().find(|(id, _)| *id == field).map(|(_, v)| *v)
            }
            _ => None,
        }
    }

    pub fn instance_put(&mut self, obj: Value, field: u32, val: Value) -> bool {
        let Value::Obj(i) = obj else {
            return false;
        };
        if i == 0 {
            return false;
        }
        match self.objects.get_mut((i - 1) as usize) {
            Some(HleObject::Instance { fields, .. }) => {
                if let Some(slot) = fields.iter_mut().find(|(id, _)| *id == field) {
                    slot.1 = val;
                } else {
                    fields.push((field, val));
                }
                true
            }
            _ => false,
        }
    }

    pub fn array_len(&self, v: Value) -> Option<i32> {
        match v {
            Value::Obj(0) | Value::Int(_) => None,
            Value::Obj(i) => match self.objects.get((i - 1) as usize) {
                Some(HleObject::Array { len, .. }) => Some(*len),
                _ => None,
            },
        }
    }

    pub fn array_get(&self, arr: Value, idx: i32) -> Option<Value> {
        match arr {
            Value::Obj(0) | Value::Int(_) => None,
            Value::Obj(i) => match self.objects.get((i - 1) as usize) {
                Some(HleObject::Array { elems, .. }) => elems.get(idx as usize).copied(),
                _ => None,
            },
        }
    }

    pub fn array_set(&mut self, arr: Value, idx: i32, val: Value) -> bool {
        match arr {
            Value::Obj(0) | Value::Int(_) => false,
            Value::Obj(i) => match self.objects.get_mut((i - 1) as usize) {
                Some(HleObject::Array { elems, .. }) => {
                    if let Some(slot) = elems.get_mut(idx as usize) {
                        *slot = val;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        }
    }

    pub fn get_string(&self, v: Value) -> Option<&str> {
        match v {
            Value::Obj(0) | Value::Int(_) => None,
            Value::Obj(i) => match self.objects.get((i - 1) as usize) {
                Some(HleObject::String(s)) => Some(s.as_str()),
                _ => None,
            },
        }
    }

    pub fn log_line(&mut self, line: impl Into<String>) {
        let line = line.into();
        self.logs.push(line.clone());
        if self.canvas_text.is_empty() {
            self.canvas_text = line;
        } else {
            self.canvas_text.push('\n');
            self.canvas_text.push_str(&line);
        }
    }

    /// Returns Some(result) if HLE handled the invoke; None to run DEX code.
    pub fn try_invoke(&mut self, method: &MethodId, args: &[Value]) -> Option<Value> {
        let class = method.class.as_str();
        let name = method.name.as_str();

        // android.util.Log.* (tag, msg) or (msg)
        if class == "Landroid/util/Log;" {
            let msg = args
                .get(1)
                .and_then(|v| self.get_string(*v).map(|s| s.to_string()))
                .or_else(|| {
                    args.first()
                        .and_then(|v| self.get_string(*v).map(|s| s.to_string()))
                })
                .unwrap_or_else(|| "<null>".into());
            let tag = args
                .first()
                .and_then(|v| self.get_string(*v).map(|s| s.to_string()))
                .unwrap_or_else(|| "App".into());
            self.log_line(format!("[{tag}] {msg}"));
            return Some(Value::Int(0));
        }

        // System.out.println
        if (class == "Ljava/io/PrintStream;" && name == "println")
            || (class == "Lnekodroid/Hle;" && name == "println")
        {
            let msg = args
                .last()
                .and_then(|v| self.get_string(*v).map(|s| s.to_string()))
                .unwrap_or_else(|| "<null>".into());
            self.log_line(msg);
            return Some(Value::Int(0));
        }

        // HLE integer print (for arith fixtures)
        if class == "Lnekodroid/Hle;" && name == "printInt" {
            let n = args.last().map(|v| v.as_int()).unwrap_or(0);
            self.log_line(format!("{n}"));
            return Some(Value::Int(0));
        }

        // TextView.setText — framework class only. An app method of the same name runs DEX.
        if is_framework_class(class) && name == "setText" && class.contains("TextView") {
            let msg = args
                .get(1)
                .or_else(|| args.first())
                .and_then(|v| self.get_string(*v).map(|s| s.to_string()))
                .unwrap_or_default();
            self.canvas_text = msg.clone();
            self.log_line(format!("TextView: {msg}"));
            return Some(Value::Int(0));
        }

        // Framework Activity hooks. App methods with these names are not stubs.
        // `<init>` is intentionally not listed: app constructors live in DEX,
        // and a missing framework `<init>` falls through to the no-code stub.
        if is_framework_class(class) && matches!(name, "setContentView" | "setTitle") {
            return Some(Value::Int(0));
        }

        if is_framework_class(class) && name == "getLastTouchX" {
            return Some(Value::Int(self.touch_x));
        }
        if is_framework_class(class) && name == "getLastTouchY" {
            return Some(Value::Int(self.touch_y));
        }
        if is_framework_class(class) && name == "isTouchDown" {
            return Some(Value::Int(if self.touch_down { 1 } else { 0 }));
        }

        None
    }
}

fn is_framework_class(class: &str) -> bool {
    class.starts_with("Landroid/")
        || class.starts_with("Ljava/")
        || class.starts_with("Ljavax/")
        || class.starts_with("Ldalvik/")
        || class.starts_with("Llibcore/")
        || class.starts_with("Lcom/android/")
}
