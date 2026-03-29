import { reactive } from "vue";

export const bingoConfig = reactive({
  // アセット設定
  assets: {
    background: 'path/to/bg.png',
    hit_mark: 'path/to/hit.png',
  },
  // グリッドエリア設定（ここが肝）
  grid_area: {
    // 画像の左上原点(0,0)からのピクセル数値
    x: 100, // 開始X座標
    y: 200, // 開始Y座標
    w: 600, // グリッド全体の幅
    h: 500  // グリッド全体の高さ
  },
  is_random: false
});