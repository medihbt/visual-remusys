import { editor } from "monaco-editor";
import type { IRObjPath } from "remusys-wasm";

export interface RenameInputState {
    path: IRObjPath;
    value: string;
}

export class RenameInputWidget implements editor.IContentWidget {
    private ed: editor.IStandaloneCodeEditor;
    private onSubmit: (state: RenameInputState) => void;
    private onCancel: () => void;
    private position: { lineNumber: number; column: number } | null = null;
    private currentPath: IRObjPath | null = null;
    private visible = false;
    private readonly domNode: HTMLDivElement;
    private readonly inputNode: HTMLInputElement;
    private readonly hintNode: HTMLDivElement;

    constructor(
        ed: editor.IStandaloneCodeEditor,
        onSubmit: (state: RenameInputState) => void,
        onCancel: () => void,
    ) {
        this.ed = ed;
        this.onSubmit = onSubmit;
        this.onCancel = onCancel;
        this.domNode = document.createElement("div");
        this.domNode.className = "source-rename-widget";

        this.inputNode = document.createElement("input");
        this.inputNode.className = "source-rename-input";
        this.inputNode.type = "text";
        this.inputNode.spellcheck = false;
        this.inputNode.addEventListener("keydown", this.handleKeyDown);

        this.hintNode = document.createElement("div");
        this.hintNode.className = "source-rename-hint";
        this.hintNode.textContent = "Enter 确认，Escape 取消";

        this.domNode.append(this.inputNode, this.hintNode);
    }

    getId(): string {
        return "remusys.rename-input-widget";
    }

    getDomNode(): HTMLElement {
        return this.domNode;
    }

    getPosition(): editor.IContentWidgetPosition | null {
        if (!this.position) {
            return null;
        }

        return {
            position: this.position,
            preference: [editor.ContentWidgetPositionPreference.BELOW],
        };
    }

    show(position: { lineNumber: number; column: number }, path: IRObjPath, initialValue: string) {
        this.position = position;
        this.currentPath = path;
        this.inputNode.value = initialValue;
        if (!this.visible) {
            document.addEventListener("pointerdown", this.handlePointerDown, true);
            this.visible = true;
        }
        this.ed.addContentWidget(this);
        this.ed.layoutContentWidget(this);
        window.setTimeout(() => {
            this.inputNode.focus();
            this.inputNode.select();
        }, 0);
    }

    hide() {
        if (this.visible) {
            document.removeEventListener("pointerdown", this.handlePointerDown, true);
            this.visible = false;
        }
        this.position = null;
        this.currentPath = null;
        this.ed.removeContentWidget(this);
    }

    dispose() {
        this.inputNode.removeEventListener("keydown", this.handleKeyDown);
        this.hide();
    }

    private readonly handleKeyDown = (event: KeyboardEvent) => {
        if (event.key === "Enter") {
            event.preventDefault();
            if (!this.currentPath) {
                this.hide();
                this.ed.focus();
                return;
            }
            this.onSubmit({ path: this.currentPath, value: this.inputNode.value });
            this.hide();
            this.ed.focus();
            return;
        }

        if (event.key === "Escape") {
            event.preventDefault();
            this.onCancel();
            this.hide();
            this.ed.focus();
        }
    };

    private readonly handlePointerDown = (event: PointerEvent) => {
        if (!this.visible) {
            return;
        }

        const target = event.target;
        if (target instanceof Node && this.domNode.contains(target)) {
            return;
        }

        this.onCancel();
        this.hide();
    };
}
