import MaskEditor from "../../components/MaskEditor";
import type { RefImage } from "../../types";

interface Props {
  image: RefImage | undefined;
  onDone: (mask: RefImage | null) => void;
  onClose: () => void;
}

/** 底图存在且打开编辑器时才挂载；MaskEditor 依赖 image 初始化画布，不能空挂载。 */
export default function MaskEditorLayer({ image, onDone, onClose }: Props) {
  if (!image) return null;
  return <MaskEditor image={image} onDone={onDone} onClose={onClose} />;
}
