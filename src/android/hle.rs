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
    Instance { class: String, fields: Vec<(u32, Value)> },
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
    pub fn try_invoke(
        &mut self,
        method: &MethodId,
        args: &[Value],
    ) -> Option<Value> {
        let class = method.class.as_str();
        let name = method.name.as_str();

        // android.util.Log.* (tag, msg) or (msg)
        if class == "Landroid/util/Log;" {
            let msg = args
                .get(1)
                .and_then(|v| self.get_string(*v).map(|s| s.to_string()))
                .or_else(|| args.first().and_then(|v| self.get_string(*v).map(|s| s.to_string())))
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

        // TextView.setText
        if name == "setText" && class.contains("TextView") {
            let msg = args
                .get(1)
                .or_else(|| args.first())
                .and_then(|v| self.get_string(*v).map(|s| s.to_string()))
                .unwrap_or_default();
            self.canvas_text = msg.clone();
            self.log_line(format!("TextView: {msg}"));
            return Some(Value::Int(0));
        }

        // Activity.<init> / Object.<init>
        if name == "<init>" {
            return Some(Value::Int(0));
        }

        // Activity.setContentView / onCreate empty stubs
        if matches!(name, "setContentView" | "setTitle") {
            return Some(Value::Int(0));
        }

        // MotionEvent-ish: Activity stubs can read last touch via HLE fields
        if name == "getLastTouchX" {
            return Some(Value::Int(self.touch_x));
        }
        if name == "getLastTouchY" {
            return Some(Value::Int(self.touch_y));
        }
        if name == "isTouchDown" {
            return Some(Value::Int(if self.touch_down { 1 } else { 0 }));
        }

        None
    }
}
