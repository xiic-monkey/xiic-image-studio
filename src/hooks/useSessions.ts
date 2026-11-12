import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../ipc";
import type { SessionItem } from "../types";

/** 会话 tab 的 CRUD 与选中态。草稿内容由 useDraft 负责，这里不碰表单。 */
export function useSessions(setNotice: (msg: string | null) => void) {
  const [sessions, setSessions] = useState<SessionItem[]>([]);
  const [sessionId, setSessionId] = useState("");
  // StrictMode 会把挂载 effect 跑两遍；两次都看到空列表就会建出两个「默认会话」。
  // ref 在模拟卸载/重挂之间保留，真正的 tab 切走再回来是新实例，仍会重新拉一次列表。
  const booted = useRef(false);

  const refresh = useCallback(async (preferId?: string) => {
    const list = await api.sessionList();
    setSessions(list);
    // preferId 缺省时选第一个（挂载 / 删除当前会话后的场景，不依赖闭包里的旧 sessionId）
    const target = preferId ?? list[0]?.id;
    if (target && list.some((s) => s.id === target)) {
      setSessionId(target);
    } else if (list.length === 0) {
      const id = await api.sessionSave({ name: "默认会话" });
      setSessions(await api.sessionList());
      setSessionId(id);
    }
  }, []);

  useEffect(() => {
    if (booted.current) return;
    booted.current = true;
    void refresh();
  }, [refresh]);

  const selectSession = useCallback((id: string) => {
    setSessionId(id);
  }, []);

  const newSession = useCallback(async () => {
    // 新会话命名：取最小的未占用编号（存在「会话 3」时新建「会话 1」）
    const names = new Set(sessions.map((s) => s.name));
    let i = 1;
    while (names.has(`会话 ${i}`)) i++;
    const id = await api.sessionSave({ name: `会话 ${i}` });
    await refresh(id);
  }, [sessions, refresh]);

  const deleteSession = useCallback(
    async (id: string) => {
      if (sessions.length <= 1) {
        setNotice("至少保留一个会话");
        return;
      }
      if (!confirm("删除该会话？（不影响已生成图片）")) return;
      await api.sessionDelete(id);
      // 删的是当前会话 → 选第一个；删的是别的 → 保持在当前
      await refresh(id === sessionId ? undefined : sessionId);
    },
    [sessions.length, sessionId, refresh, setNotice],
  );

  const renameSession = useCallback(
    async (id: string, name: string) => {
      if (!name.trim()) return;
      await api.sessionSave({ id, name });
      await refresh(id);
    },
    [refresh],
  );

  return { sessions, sessionId, selectSession, newSession, deleteSession, renameSession, refresh };
}
