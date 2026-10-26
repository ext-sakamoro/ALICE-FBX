# Changelog

この file には主な変更を記録する
書式は [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)、版は [Semantic Versioning](https://semver.org/) に従う

## [Unreleased]

### Added

- `read_tree`: binary FBX (版 7100-7700) の node tree を `Document` / `Node` / `Prop` で返す 全ての property の型と、生および zlib の配列を読む
- `read_scene`: node tree から scene を作る: `Model` (骨・mesh・Null、移動・回転・拡縮・前後の回転・pivot と 6 つの回転順、親子、`local_matrix` / `Scene::world_matrix`)、`Mesh` (点、多角形、corner ごとの法線と UV、多角形ごとの材質、`triangles`)、skin の cluster、blend shape の channel と target、`Material`、軸と単位 FullWeights は target ごとの形と、1 target の頂点ごとに等しい値の形を読む
- 合わない file は panic せず offset を添えた `FbxError` で拒否する (範囲外の offset・長さ、null record で閉じない列、宣言と違う長さの配列、256 MiB を超える配列、64 段を超える入れ子、scene では範囲外の index・長さの合わない配列・閉じない多角形・有限でない数・読まない mapping や回転順・存在しない object への接続・親子の循環)
