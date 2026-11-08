import { useCallback, useEffect, useRef, useState } from "react";
import Feed from "../components/Feed";
import { api } from "../api";
import type { VideoCard } from "../types";
import { useStore } from "../store";
import { ISearch, IVol, IVolMute } from "../icons";
import { Empty } from "../components/ui";

export default function Home({ active, bottomInset }: { active: boolean; bottomInset: number }) {
  const { muted, setMuted, setTab, toast, fs } = useStore();
  const [items, setItems] = useState<VideoCard[]>([]);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(false);
  const [done, setDone] = useState(false);
  const [error, setError] = useState("");
  const loadingRef = useRef(false);

  const load = useCallback(async (p: number) => {
    if (loadingRef.current || done) return;
    loadingRef.current = true;
    setLoading(true);
    setError("");
    try {
      const res = await api.feed(p, 10);
      const list = res?.items || [];
      setItems((prev) => {
        const seen = new Set(prev.map((x) => x.id));
        return [...prev, ...list.filter((x) => !seen.has(x.id))];
      });
      setPage(p);
      if (!list.length || (res?.pagination && p >= res.pagination.pages)) setDone(true);
    } catch (e) {
      const message = (e as Error).message || "加载失败";
      setError(message);
      toast(message, "info");
    } finally {
      loadingRef.current = false;
      setLoading(false);
    }
  }, [done, toast]);

  useEffect(() => { if (!items.length) load(1); /* eslint-disable-next-line */ }, []);

  const ids = items.map((v) => v.id);

  return (
    <div className="absolute inset-0 bg-black">
      {items.length ? (
        <Feed videoIds={ids} mode="home" active={active} bottomInset={bottomInset}
          onNearEnd={() => { if (!loading && !done) load(page + 1); }} />
      ) : !loading && error ? (
        <div className="absolute inset-0 flex items-center justify-center bg-bg">
          <Empty icon={<ISearch size={26} />} title="首页加载失败" sub={error} action="重试" onAction={() => void load(1)} />
        </div>
      ) : !loading && done ? (
        <div className="absolute inset-0 flex items-center justify-center bg-bg"><Empty icon={<ISearch size={26} />} title="暂无视频" /></div>
      ) : (
        <div className="absolute inset-0 flex items-center justify-center"><div className="w-9 h-9 spinner" /></div>
      )}

      {/* 移动端：静音 + 搜索（压在视频上，固定白字 + 半透明黑底） */}
      {!fs && (
        <div className="absolute inset-x-0 z-20 flex items-center px-3 pointer-events-none hg-player-ui" style={{ top: "calc(var(--sat, 0px) + 6px)" }}>
          <div className="ml-auto flex items-center gap-2">
            <button onClick={() => setMuted((m) => !m)} aria-label={muted ? "取消静音" : "静音"}
              className="hg-chip pointer-events-auto w-9 h-9 rounded-full flex items-center justify-center pressable elev-1">
              {muted ? <IVolMute size={17} /> : <IVol size={17} />}
            </button>
            <button onClick={() => setTab("search")} aria-label="搜索"
              className="hg-chip pointer-events-auto w-9 h-9 rounded-full flex items-center justify-center pressable elev-1">
              <ISearch size={18} />
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
