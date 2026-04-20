import type { MouseEvent } from "react";
import type { GuideNodeData } from "remusys-wasm-b2";

export type NodeMenuItem = {
	label: string;
	onSelect: (node: GuideNodeData) => void;
	disabled?: boolean;
};

export type NodeMenuProps = {
	x: number;
	y: number;
	node: GuideNodeData;
	items: NodeMenuItem[];
	onClose: () => void;
	width?: number;
};

function clampMenuPosition(x: number, y: number, width: number, itemCount: number) {
	const screenWidth = window.innerWidth;
	const screenHeight = window.innerHeight;
	const rowHeight = 40;
	const menuHeight = (itemCount + 1) * rowHeight;

	const left = Math.max(8, Math.min(x, screenWidth - width - 8));
	const top = Math.max(8, Math.min(y, screenHeight - menuHeight - 8));
	return { left, top };
}

export function NodeMenu({
	x,
	y,
	node,
	items,
	onClose,
	width = 200,
}: NodeMenuProps) {
	const { left, top } = clampMenuPosition(x, y, width, items.length);

	function handleMenuClick(e: MouseEvent<HTMLDivElement>) {
		e.stopPropagation();
	}

	function handleSelect(item: NodeMenuItem) {
		if (item.disabled) {
			return;
		}
		item.onSelect(node);
		onClose();
	}

	return (
		<div
			style={{
				position: "fixed",
				left: `${left}px`,
				top: `${top}px`,
				width: `${width}px`,
				backgroundColor: "#ffffff",
				border: "1px solid #e5e7eb",
				borderRadius: "8px",
				boxShadow: "0 4px 12px rgba(0,0,0,0.15)",
				zIndex: 1000,
				fontFamily: "system-ui, sans-serif",
				overflow: "hidden",
				userSelect: "none",
			}}
			onClick={handleMenuClick}
		>
			{items.map((item, idx) => (
				<div
					key={`${item.label}-${idx}`}
					onClick={() => handleSelect(item)}
					style={{
						padding: "10px 15px",
						cursor: item.disabled ? "not-allowed" : "pointer",
						fontSize: "13px",
						color: item.disabled ? "#9ca3af" : "#4b5563",
						backgroundColor: "#ffffff",
						transition: "background-color 0.1s",
						borderTop: idx === 0 ? "none" : "1px solid #e5e7eb",
					}}
					onMouseEnter={(e) => {
						if (!item.disabled) {
							e.currentTarget.style.backgroundColor = "#f3f4f6";
						}
					}}
					onMouseLeave={(e) => {
						e.currentTarget.style.backgroundColor = "#ffffff";
					}}
				>
					{item.label}
				</div>
			))}

			<div
				onClick={onClose}
				style={{
					padding: "10px 15px",
					cursor: "pointer",
					fontSize: "13px",
					color: "#9ca3af",
					borderTop: "1px solid #e5e7eb",
					textAlign: "center",
				}}
			>
				取消
			</div>
		</div>
	);
}

export default NodeMenu;
