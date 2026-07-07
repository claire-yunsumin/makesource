//! ComfyUI 워크플로 슬롯 치환기 (TAD §6).
//!
//! 노드 ID를 하드코딩하지 않는다 — 템플릿의 각 노드에 붙은 `"_slot": "<이름>"`
//! 커스텀 키를 찾아 값을 주입하고, 출력에서 `_slot` 키를 제거한다.
//! 결과는 `POST /prompt` 페이로드(`{"prompt": <workflow>, "client_id": ...}`)로 감싼다.

use serde_json::{json, Map, Value};

use crate::error::AppError;

/// 내장 워크플로 템플릿 3종 (TAD §6 필수 목록).
pub const SDXL_BASE: &str = include_str!("../../resources/workflows/sdxl_base.json");
pub const SDXL_LORA: &str = include_str!("../../resources/workflows/sdxl_lora.json");
pub const SDXL_IPADAPTER: &str = include_str!("../../resources/workflows/sdxl_ipadapter.json");

/// LoRA 주입 파라미터 (경로·weight 파라미터화 — TAD §6).
#[derive(Debug, Clone)]
pub struct LoraParams {
    /// models/loras/ 안의 파일명 (ComfyUI 기준 상대명)
    pub lora_name: String,
    pub weight: f64,
}

/// IP-Adapter 주입 파라미터 (weight 파라미터화 — TAD §6).
#[derive(Debug, Clone)]
pub struct IpAdapterParams {
    /// ComfyUI input 폴더 기준 참조 이미지 이름
    pub image: String,
    pub weight: f64,
}

/// 슬롯에 주입할 값 묶음.
#[derive(Debug, Clone)]
pub struct WorkflowParams {
    pub prompt: String,
    pub negative: String,
    pub seed: i64,
    pub steps: u32,
    pub cfg: f64,
    pub width: u32,
    pub height: u32,
    pub batch: u32,
    /// None이면 템플릿 기본 체크포인트 유지
    pub checkpoint: Option<String>,
    /// lora 템플릿에서 필수
    pub lora: Option<LoraParams>,
    /// ipadapter 템플릿에서 필수
    pub ipadapter: Option<IpAdapterParams>,
}

/// 템플릿 파싱 캐시 (T9.2, docs/11 §P1.5) — 내장 템플릿 3종이 생성마다
/// 재파싱되는 것을 막는다. 키는 내용 문자열 자체 (CLAUDE.md 주의: `include_str!`
/// const는 사용처마다 인라인되므로 포인터가 아니라 내용으로 비교).
fn parse_template_cached(template_json: &str) -> Result<Map<String, Value>, AppError> {
    use std::collections::HashMap;
    static CACHE: std::sync::Mutex<Option<HashMap<String, Map<String, Value>>>> =
        std::sync::Mutex::new(None);

    if let Ok(guard) = CACHE.lock() {
        if let Some(map) = guard.as_ref().and_then(|c| c.get(template_json)) {
            return Ok(map.clone());
        }
    }
    let workflow: Map<String, Value> = serde_json::from_str(template_json).map_err(|e| {
        AppError::with_detail("E_TEMPLATE_PARSE", "워크플로 템플릿을 읽지 못했어요.", e)
    })?;
    if let Ok(mut guard) = CACHE.lock() {
        guard
            .get_or_insert_with(HashMap::new)
            .insert(template_json.to_string(), workflow.clone());
    }
    Ok(workflow)
}

/// 템플릿 JSON에 슬롯 값을 주입해 완성된 워크플로를 만든다.
/// - 알 수 없는 슬롯 이름 → E_SLOT_UNKNOWN
/// - 필요한 값이 없는 슬롯(lora/ipadapter 등) → E_SLOT_MISSING
/// - 출력에는 `_slot` 키가 남지 않는다
pub fn apply_slots(template_json: &str, params: &WorkflowParams) -> Result<Value, AppError> {
    let mut workflow = parse_template_cached(template_json)?;

    for (node_id, node) in workflow.iter_mut() {
        let Some(node_obj) = node.as_object_mut() else {
            return Err(AppError::with_detail(
                "E_TEMPLATE_PARSE",
                "워크플로 템플릿 형식이 올바르지 않아요.",
                format!("노드 {node_id}가 객체가 아님"),
            ));
        };
        // _slot 키를 제거하면서 읽는다 (출력에 남기지 않음)
        let Some(slot_value) = node_obj.remove("_slot") else {
            continue;
        };
        let Some(slot) = slot_value.as_str() else {
            return Err(AppError::with_detail(
                "E_TEMPLATE_PARSE",
                "워크플로 템플릿 형식이 올바르지 않아요.",
                format!("노드 {node_id}의 _slot이 문자열이 아님"),
            ));
        };

        let inputs = node_obj
            .get_mut("inputs")
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| {
                AppError::with_detail(
                    "E_TEMPLATE_PARSE",
                    "워크플로 템플릿 형식이 올바르지 않아요.",
                    format!("노드 {node_id}에 inputs가 없음"),
                )
            })?;

        apply_one_slot(slot, inputs, params).map_err(|mut e| {
            e.detail = Some(format!(
                "노드 {node_id} (slot={slot}): {}",
                e.detail.unwrap_or_default()
            ));
            e
        })?;
    }

    Ok(Value::Object(workflow))
}

/// 슬롯 이름 → inputs 필드 주입 규칙 (TAD §6의 slot 어휘).
fn apply_one_slot(
    slot: &str,
    inputs: &mut Map<String, Value>,
    params: &WorkflowParams,
) -> Result<(), AppError> {
    match slot {
        "prompt" => {
            inputs.insert("text".into(), json!(params.prompt));
        }
        "negative" => {
            inputs.insert("text".into(), json!(params.negative));
        }
        "sampler" => {
            inputs.insert("seed".into(), json!(params.seed));
            inputs.insert("steps".into(), json!(params.steps));
            inputs.insert("cfg".into(), json!(params.cfg));
        }
        "latent" => {
            inputs.insert("width".into(), json!(params.width));
            inputs.insert("height".into(), json!(params.height));
            inputs.insert("batch_size".into(), json!(params.batch));
        }
        "checkpoint" => {
            // 지정 시에만 덮어씀 (템플릿 기본값 허용)
            if let Some(ckpt) = &params.checkpoint {
                inputs.insert("ckpt_name".into(), json!(ckpt));
            }
        }
        "lora" => {
            let lora = params.lora.as_ref().ok_or_else(|| {
                AppError::with_detail(
                    "E_SLOT_MISSING",
                    "LoRA 스타일 정보가 없어요.",
                    "lora 슬롯에 주입할 값 없음",
                )
            })?;
            inputs.insert("lora_name".into(), json!(lora.lora_name));
            inputs.insert("strength_model".into(), json!(lora.weight));
            inputs.insert("strength_clip".into(), json!(lora.weight));
        }
        "reference_image" => {
            let ip = params.ipadapter.as_ref().ok_or_else(|| {
                AppError::with_detail(
                    "E_SLOT_MISSING",
                    "참조 이미지가 없어요.",
                    "reference_image 슬롯에 주입할 값 없음",
                )
            })?;
            inputs.insert("image".into(), json!(ip.image));
        }
        "ipadapter" => {
            let ip = params.ipadapter.as_ref().ok_or_else(|| {
                AppError::with_detail(
                    "E_SLOT_MISSING",
                    "스타일 강도 정보가 없어요.",
                    "ipadapter 슬롯에 주입할 값 없음",
                )
            })?;
            inputs.insert("weight".into(), json!(ip.weight));
        }
        other => {
            return Err(AppError::with_detail(
                "E_SLOT_UNKNOWN",
                "워크플로 템플릿에 알 수 없는 슬롯이 있어요.",
                format!("unknown slot: {other}"),
            ));
        }
    }
    Ok(())
}

/// `POST /prompt` 페이로드로 감싼다 (TAD §6).
pub fn build_prompt_payload(workflow: Value, client_id: &str) -> Value {
    json!({
        "prompt": workflow,
        "client_id": client_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_params() -> WorkflowParams {
        WorkflowParams {
            prompt: "cinematic illustration of a log cabin".into(),
            negative: "text, watermark".into(),
            seed: 42,
            steps: 28,
            cfg: 6.5,
            width: 1024,
            height: 1024,
            batch: 4,
            checkpoint: None,
            lora: None,
            ipadapter: None,
        }
    }

    /// 치환 결과에서 특정 class_type 노드의 inputs를 찾는다 (노드 ID 비의존 — TAD §6 취지).
    fn find_inputs<'a>(workflow: &'a Value, class_type: &str) -> Vec<&'a Value> {
        workflow
            .as_object()
            .unwrap()
            .values()
            .filter(|n| n["class_type"] == class_type)
            .map(|n| &n["inputs"])
            .collect()
    }

    fn assert_no_slot_keys(workflow: &Value) {
        for (id, node) in workflow.as_object().unwrap() {
            assert!(
                node.get("_slot").is_none(),
                "노드 {id}에 _slot 키가 남아 있음"
            );
        }
    }

    /// 모든 노드 참조(["id", n])가 실제 존재하는 노드를 가리키는지 검증.
    fn assert_refs_valid(workflow: &Value) {
        let nodes = workflow.as_object().unwrap();
        for (id, node) in nodes {
            for (key, input) in node["inputs"].as_object().unwrap() {
                if let Some(arr) = input.as_array() {
                    if arr.len() == 2 && arr[0].is_string() {
                        let target = arr[0].as_str().unwrap();
                        assert!(
                            nodes.contains_key(target),
                            "노드 {id}.{key}가 존재하지 않는 노드 {target} 참조"
                        );
                    }
                }
            }
        }
    }

    // ---- AC: 템플릿 → 유효한 /prompt 페이로드 ----

    #[test]
    fn sdxl_base_produces_valid_payload() {
        let wf = apply_slots(SDXL_BASE, &base_params()).unwrap();
        assert_no_slot_keys(&wf);
        assert_refs_valid(&wf);

        // 프롬프트/네거티브가 CLIPTextEncode에 주입됨
        let texts: Vec<_> = find_inputs(&wf, "CLIPTextEncode")
            .iter()
            .map(|i| i["text"].as_str().unwrap().to_string())
            .collect();
        assert!(texts.contains(&"cinematic illustration of a log cabin".to_string()));
        assert!(texts.contains(&"text, watermark".to_string()));

        // 샘플러 파라미터
        let sampler = find_inputs(&wf, "KSampler")[0];
        assert_eq!(sampler["seed"], 42);
        assert_eq!(sampler["steps"], 28);
        assert_eq!(sampler["cfg"], 6.5);

        // 해상도/장수
        let latent = find_inputs(&wf, "EmptyLatentImage")[0];
        assert_eq!(latent["width"], 1024);
        assert_eq!(latent["batch_size"], 4);

        // /prompt 페이로드 형태
        let payload = build_prompt_payload(wf, "job-1");
        assert_eq!(payload["client_id"], "job-1");
        assert!(payload["prompt"].is_object());
    }

    #[test]
    fn sdxl_lora_injects_path_and_weight() {
        let mut params = base_params();
        params.lora = Some(LoraParams {
            lora_name: "brand_v1.safetensors".into(),
            weight: 0.7,
        });
        let wf = apply_slots(SDXL_LORA, &params).unwrap();
        assert_no_slot_keys(&wf);
        assert_refs_valid(&wf);

        let lora = find_inputs(&wf, "LoraLoader")[0];
        assert_eq!(lora["lora_name"], "brand_v1.safetensors");
        assert_eq!(lora["strength_model"], 0.7);
        assert_eq!(lora["strength_clip"], 0.7);
    }

    #[test]
    fn sdxl_ipadapter_injects_weight_and_image() {
        let mut params = base_params();
        params.ipadapter = Some(IpAdapterParams {
            image: "ref_01.png".into(),
            weight: 0.6,
        });
        let wf = apply_slots(SDXL_IPADAPTER, &params).unwrap();
        assert_no_slot_keys(&wf);
        assert_refs_valid(&wf);

        assert_eq!(find_inputs(&wf, "IPAdapter")[0]["weight"], 0.6);
        assert_eq!(find_inputs(&wf, "LoadImage")[0]["image"], "ref_01.png");
    }

    // ---- 에러 경로 ----

    #[test]
    fn lora_template_without_lora_params_fails() {
        let err = apply_slots(SDXL_LORA, &base_params()).unwrap_err();
        assert_eq!(err.code, "E_SLOT_MISSING");
        // 어떤 노드/슬롯인지 detail에 남는다
        assert!(err.detail.unwrap().contains("slot=lora"));
    }

    #[test]
    fn ipadapter_template_without_params_fails() {
        let err = apply_slots(SDXL_IPADAPTER, &base_params()).unwrap_err();
        assert_eq!(err.code, "E_SLOT_MISSING");
    }

    #[test]
    fn unknown_slot_is_rejected() {
        let template = r#"{ "1": { "class_type": "X", "_slot": "nope", "inputs": {} } }"#;
        let err = apply_slots(template, &base_params()).unwrap_err();
        assert_eq!(err.code, "E_SLOT_UNKNOWN");
    }

    #[test]
    fn corrupt_template_is_rejected() {
        let err = apply_slots("{not json", &base_params()).unwrap_err();
        assert_eq!(err.code, "E_TEMPLATE_PARSE");
    }

    #[test]
    fn checkpoint_override_is_optional() {
        // 미지정 → 템플릿 기본값 유지
        let wf = apply_slots(SDXL_BASE, &base_params()).unwrap();
        assert_eq!(
            find_inputs(&wf, "CheckpointLoaderSimple")[0]["ckpt_name"],
            "sd_xl_base_1.0.safetensors"
        );
        // 지정 → 덮어씀 (SD1.5 폴백 경로에서 사용 — T1.5)
        let mut params = base_params();
        params.checkpoint = Some("v1-5-pruned-emaonly-fp16.safetensors".into());
        let wf = apply_slots(SDXL_BASE, &params).unwrap();
        assert_eq!(
            find_inputs(&wf, "CheckpointLoaderSimple")[0]["ckpt_name"],
            "v1-5-pruned-emaonly-fp16.safetensors"
        );
    }

    // ---- 템플릿 자체 검증 (3종 필수 슬롯 존재) ----

    #[test]
    fn all_templates_declare_required_slots() {
        for (name, tpl, extra) in [
            ("sdxl_base", SDXL_BASE, vec![]),
            ("sdxl_lora", SDXL_LORA, vec!["lora"]),
            (
                "sdxl_ipadapter",
                SDXL_IPADAPTER,
                vec!["ipadapter", "reference_image"],
            ),
        ] {
            let parsed: Value = serde_json::from_str(tpl).unwrap();
            let slots: Vec<String> = parsed
                .as_object()
                .unwrap()
                .values()
                .filter_map(|n| n["_slot"].as_str().map(String::from))
                .collect();
            for required in ["prompt", "negative", "sampler", "latent", "checkpoint"]
                .into_iter()
                .chain(extra)
            {
                assert!(
                    slots.iter().any(|s| s == required),
                    "{name} 템플릿에 {required} 슬롯이 없음"
                );
            }
        }
    }
}
