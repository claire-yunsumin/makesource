-- 성능 보조 인덱스 (T9.2, docs/11 §P1.3)
-- history_list의 ♥/스타일 필터가 created_at 인덱스만으로는 풀스캔이 되는 문제.
-- favorite는 1인 행이 소수라 부분 인덱스로.
CREATE INDEX idx_gen_fav ON generations (favorite, created_at DESC, id DESC)
  WHERE favorite = 1;
CREATE INDEX idx_gen_style ON generations (style_id, created_at DESC, id DESC);
