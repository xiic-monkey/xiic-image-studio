interface Props {
  x: number;
  y: number;
  onRename: () => void;
  onDelete: () => void;
}

export default function SessionContextMenu({ x, y, onRename, onDelete }: Props) {
  return (
    <div
      className="fixed z-[100] min-w-[120px] rounded-md border border-neutral-200 bg-white py-1 shadow-lg dark:border-neutral-700 dark:bg-neutral-900"
      style={{ left: x, top: y }}
      onMouseDown={(e) => e.stopPropagation()}
    >
      <button
        className="block w-full px-3 py-1.5 text-left text-xs text-neutral-700 hover:bg-neutral-100 dark:text-neutral-200 dark:hover:bg-neutral-800"
        onClick={onRename}
      >重命名</button>
      <button
        className="block w-full px-3 py-1.5 text-left text-xs text-red-600 hover:bg-red-50 dark:hover:bg-red-950"
        onClick={onDelete}
      >删除</button>
    </div>
  );
}
