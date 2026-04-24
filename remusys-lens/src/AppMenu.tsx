import { Menu, MenuButton, MenuItem, MenuItems } from '@headlessui/react';
import type { SourceTy } from 'remusys-wasm';
import { handleFileLoad } from './file-load';

const menuStyle = {
  height: 28,
  background: '#ececec',
  color: '#000000',
  display: 'flex',
  alignItems: 'center',
  padding: '0 8px',
  fontSize: 12,
};
const btnStyle = {
  background: 'transparent',
  color: 'inherit',
  border: 'none',
  padding: '6px 10px',
  cursor: 'pointer',
};
const itemsStyle: React.CSSProperties = {
  position: 'absolute',
  top: 28,
  left: 0,
  background: '#fff',
  color: '#000',
  boxShadow: '0 2px 8px rgba(0,0,0,0.2)',
  minWidth: 160,
  zIndex: 1000,
};
function itemStyle(active: boolean) {
  const bg = active ? '#f3f4f6' : 'transparent';
  return {
    padding: '8px 12px',
    cursor: 'pointer',
    background: bg,
  };
}

const aboutText = `Visual Remusys
A visualization tool for Remusys IR, built with React.

Features:
- Interactive CFG and DFG views
- Source code navigation
- Extensible architecture
`;

export type TopMenuActions = {
  onLoad: (mode: SourceTy, text: string, filename: string) => void;
};
export default function AppMenu(actions: TopMenuActions) {
  function about() {
    alert(aboutText);
  }
  function open() {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.ll,.ir,.remusys-ir,.sy,.sysy';
    input.onchange = () => {
      const file = input.files?.[0];
      if (!file) return;
      handleFileLoad(file, actions.onLoad);
    };
    input.click();
  }

  return (
    <div style={menuStyle}>
      <Menu as="div" style={{ position: 'relative' }}>
        <MenuButton style={btnStyle}>文件</MenuButton>
        <MenuItems style={itemsStyle}>
          <MenuItem>
            {({ focus }) => (<div onClick={open} style={itemStyle(focus)}>打开...</div>)}
          </MenuItem>
        </MenuItems>
      </Menu>

      <Menu as="div" style={{ position: 'relative', marginLeft: 8 }}>
        <MenuButton style={btnStyle}>帮助</MenuButton>
        <MenuItems style={itemsStyle}>
          <MenuItem>
            {({ focus }) => (
              <div onClick={about} style={itemStyle(focus)}>关于</div>
            )}
          </MenuItem>
        </MenuItems>
      </Menu>

      <div style={{ marginLeft: 'auto', paddingRight: 12, opacity: 0.8 }}>Visual Remusys</div>
    </div>
  );
}
