use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &str = include_str!("../api/types.ts");

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "SourceTy")]
    pub type SourceTy;

    #[wasm_bindgen(typescript_type = "GlobalID")]
    pub type GlobalID;

    #[wasm_bindgen(typescript_type = "BlockID")]
    pub type BlockID;

    #[wasm_bindgen(typescript_type = "InstID")]
    pub type InstID;

    #[wasm_bindgen(typescript_type = "ExprID")]
    pub type ExprID;

    #[wasm_bindgen(typescript_type = "UseID")]
    pub type UseID;

    #[wasm_bindgen(typescript_type = "JumpTargetID")]
    pub type JumpTargetID;

    #[wasm_bindgen(typescript_type = "PoolStrID")]
    pub type PoolStrID;

    #[wasm_bindgen(typescript_type = "MonacoSrcPos")]
    pub type JsMonacoSrcPos;

    #[wasm_bindgen(typescript_type = "MonacoSrcRange")]
    pub type JsMonacoSrcRange;

    #[wasm_bindgen(typescript_type = "APIntDt")]
    pub type APIntDt;

    #[wasm_bindgen(typescript_type = "ValTypeID")]
    pub type ValTypeID;

    #[wasm_bindgen(typescript_type = "AggrType")]
    pub type AggrType;

    #[wasm_bindgen(typescript_type = "ValueDt")]
    pub type ValueDt;

    #[wasm_bindgen(typescript_type = "IRTreeObjID")]
    pub type JsTreeObjID;

    #[wasm_bindgen(typescript_type = "IRObjPath")]
    pub type JsIRObjPath;

    #[wasm_bindgen(typescript_type = "IRTreeNodeClass")]
    pub type JsIRTreeNodeClass;

    #[wasm_bindgen(typescript_type = "IRTreeNodeDt")]
    pub type JsIRTreeNodeDt;

    #[wasm_bindgen(typescript_type = "IRTreeNodes")]
    pub type JsIRTreeNodes;

    #[wasm_bindgen(typescript_type = "CfgNodeRole")]
    pub type JsCfgNodeRole;

    #[wasm_bindgen(typescript_type = "CfgNodeDt")]
    pub type JsCfgNodeDt;

    #[wasm_bindgen(typescript_type = "CfgEdgeDfsRole")]
    pub type JsCfgEdgeDfsRole;

    #[wasm_bindgen(typescript_type = "JumpTargetKind")]
    pub type JsJumpTargetKind;

    #[wasm_bindgen(typescript_type = "CfgEdgeDt")]
    pub type JsCfgEdgeDt;

    #[wasm_bindgen(typescript_type = "FuncCfgDt")]
    pub type JsFuncCfgDt;

    #[wasm_bindgen(typescript_type = "DfgNodeID")]
    pub type JsDfgNodeID;

    #[wasm_bindgen(typescript_type = "DfgNodeRole")]
    pub type JsDfgNodeRole;

    #[wasm_bindgen(typescript_type = "DfgNode")]
    pub type JsDfgNode;

    #[wasm_bindgen(typescript_type = "DfgSection")]
    pub type JsDfgSection;

    #[wasm_bindgen(typescript_type = "DfgEdge")]
    pub type JsDfgEdge;

    #[wasm_bindgen(typescript_type = "BlockDfg")]
    pub type JsBlockDfg;

    #[wasm_bindgen(typescript_type = "DefUseGraph")]
    pub type JsDefUseGraph;

    #[wasm_bindgen(typescript_type = "CallGraphNodeRole")]
    pub type JsCallGraphNodeRole;

    #[wasm_bindgen(typescript_type = "CallGraphNodeDt")]
    pub type JsCallGraphNodeDt;

    #[wasm_bindgen(typescript_type = "CallGraphEdgeDt")]
    pub type JsCallGraphEdgeDt;

    #[wasm_bindgen(typescript_type = "CallGraphDt")]
    pub type JsCallGraphDt;

    #[wasm_bindgen(typescript_type = "DomTreeDt")]
    pub type JsDomTreeDt;

    #[wasm_bindgen(typescript_type = "RenameRes")]
    pub type JsRenameRes;

    #[wasm_bindgen(typescript_type = "FocusClass")]
    pub type JsFocusClass;

    #[wasm_bindgen(typescript_type = "GuideNodeData")]
    pub type JsGuideNodeData;
}
