import * as ReactFlow from "@xyflow/react";
import '@xyflow/react/dist/style.css'
import React from "react";
import { layoutCfgDot, CFG_DOT_TEXT, extractLayoutsFromGraphviz } from './layout'

export type FlowEdgeProps = ReactFlow.Edge<any> & {
    data: {
        mainPaths: string[]
        arrowPaths: string[]
    }
};

// Use any for edge props to avoid tight coupling with library Edge generic
const RoutedFlowEdge: React.FC<FlowEdgeProps> = (props) => {
    const { id, data } = props;
    const main: string[] = Array.isArray(data?.mainPaths) ? data.mainPaths : []
    const arrows: string[] = Array.isArray(data?.arrowPaths) ? data.arrowPaths : []

    const gid = String(id ?? '')

    const mainElems = main.map((path, idx) => (
        <path key={`m-${idx}`} id={`${gid}-main-${idx}`} d={path} stroke="#222" strokeWidth={1} fill="none" />
    ))
    const arrowElems = arrows.map((path, idx) => (
        <path key={`a-${idx}`} id={`${gid}-arrow-${idx}`} d={path} stroke="#222" strokeWidth={1} fill="#222" />
    ))

    return (<g id={gid} pointerEvents="none">{mainElems}{arrowElems}</g>);
}

export type FlowViewerProps = ReactFlow.Node<any> & {}

export default function FlowViewer() {
    const edgeTypes = React.useMemo(() => ({ routed: RoutedFlowEdge }), [])

    const [nodes, setNodes] = React.useState<ReactFlow.Node<any>[]>([])
    const [edges, setEdges] = React.useState<ReactFlow.Edge<any>[]>([])

    React.useEffect(() => {
        let mounted = true;
        (async () => {
            try {
                const json = await layoutCfgDot(CFG_DOT_TEXT)
                const { nodes: nlayout, edges: elayout } = extractLayoutsFromGraphviz(json)

                const rfNodes: ReactFlow.Node<any>[] = nlayout.map(n => ({
                    id: n.id,
                    position: { x: n.x - n.width / 2, y: n.y - n.height / 2 },
                    data: { label: n.id },
                    draggable: false,
                    selectable: true,
                    style: { width: `${n.width}px`, height: `${n.height}px` }
                }))

                const rfEdges: ReactFlow.Edge<any>[] = elayout
                    .filter(e => e.source && e.target)
                    .map(e => ({
                        id: e.id,
                        source: e.source!,
                        target: e.target!,
                        type: 'routed',
                        data: { mainPaths: e.mainPaths, arrowPaths: e.arrowPaths }
                    }))

                if (!mounted) return
                setNodes(rfNodes)
                setEdges(rfEdges)
            } catch (err) {
                console.error('layout error', err)
            }
        })()
        return () => { mounted = false }
    }, [])

    return (
        <ReactFlow.ReactFlow nodes={nodes} edges={edges} edgeTypes={edgeTypes} fitView >
            <ReactFlow.Background />
            <ReactFlow.Controls />
        </ReactFlow.ReactFlow>
    )
}
