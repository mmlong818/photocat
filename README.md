<div align="center">
  <img src="docs/public/icon.png" alt="猫叔的图" width="120" style="border-radius: 20px">
  <h1>猫叔的图</h1>
  <h3>离线优先的本地照片管理器 · macOS / Windows / Linux</h3>
</div>

简体中文 | [English](#english)

猫叔的图是一个完全离线的本地照片管理器，面向大体量的个人与家庭照片库。
不强制上传、不需要账号，全部索引、搜索和人脸识别都在本机完成。

> **本项目基于开源项目 [Lap](https://github.com/julyx10/lap) 开发**（作者 julyx10，GPL-3.0-or-later）。
> 索引、缩略图、RAW 解码、语义搜索、人脸聚类等核心引擎来自上游；
> 界面、交互与其上的工作流层由本项目开发。
> 详见 [NOTICE](NOTICE)。

## 主要能力

- **灵活的浏览方式**：时间线、文件夹、地点、相机、镜头、标签、收藏、评分、主题、人物。
- **智能相册**：把筛选规则存下来，带自定义分组、排序与顺序。
- **集合**：把照片归入任意集合，不移动也不复制原文件。
- **本地 AI 搜索**：文字描述搜图、以图搜图、主题分类、人脸聚类，可选下载支持 50 多种语言的多语言模型。
- **系统文件关联**：在资源管理器里双击或右键用本程序打开图片，直接定位到所在文件夹并打开查看器。
- **实况照片**：识别 HEIC 与 MOV 配对的 Apple 实况照片，播放并在改名、移动、复制、删除时保持组件不失散。
- **RAW + JPEG 配对**：可选把同名的 RAW 与 JPEG/HEIC 显示为一项，文件操作时成对处理。
- **文件夹优先**：多图库、拖放导入、粘贴导入、文件系统同步、安全的移动与删除。
- **筛片与对比**：评分、保留与淘汰标记，支持四格对比查看。
- **清理工具**：查找完全重复与高度相似的照片，批量移入回收站。
- **内置编辑**：裁剪、旋转、翻转、缩放和基础影调调整。
- **广泛的格式支持**：60 多种照片、RAW 与视频格式。

## 元数据存在哪里

本程序以文件夹为中心，但界面上显示的信息并非全部写在原文件里。如果你同时也在
访达、资源管理器或其他照片软件里管理同一批文件夹，这个区别很重要。

**跟着文件走的**

- 原始照片和视频始终是普通文件，留在原来的文件夹里。
- 文件里已有的元数据，比如 EXIF 拍摄时间、相机、镜头、GPS、方向，索引时从文件读取。
- 保存内置编辑结果时，图像写入你选择的目标位置。
- 在本程序内改名、移动、复制、删除时，目录会同步更新，并保持实况照片组件、
  AAE 附属文件、启用了配对的 RAW + JPEG/HEIC 不失散。

**只存在本地数据库里的**

以下属于图库数据，存在本地数据库和图库配置中，不写入 EXIF、IPTC 或 XMP 附属文件：

- 集合、标签、备注、收藏、评分、筛选标记（保留与淘汰）
- 智能相册及其规则、分组、排序、顺序
- AI 搜索数据、人脸数据、缩略图和其他索引缓存

这些数据不会随文件被复制或移动到别处，其他软件也读不到。
如果你依赖这些信息，尽量在本程序内做改名和移动；同时在
**设置 → 存储** 里管理数据库位置并定期备份。

删除数据库或配置只会清掉这些本地整理信息和索引，不会删除你的原始照片。

## 支持的格式

| 类型 | 格式 |
| :--- | :--- |
| 图片 | JPG/JPEG/JFIF, PNG, GIF, BMP, TIFF, WebP, HEIC/HEIF/HIF, AVIF, JXL, PSD, EXR, HDR/RGBE, TGA, JPEG 2000 (JP2/J2K/J2C/JPC/JPF/JPX), DDS, DPX, QOI, ICO, Netpbm (PBM/PGM/PPM/PAM), FITS (FITS/FIT/FTS) |
| RAW | CR2, CR3, CRW, NEF, NRW, ARW, SRF, SR2, RAF, RW2, ORF, PEF, DNG, SRW, RWL, MRW, 3FR, MOS, DCR, KDC, ERF, MEF, RAW, MDC |
| 视频 | MP4, MOV, M4V, MKV, AVI, FLV, TS/M2TS, WMV, WebM, 3GP/3G2, F4V, VOB, MPG/MPEG, ASF, DIVX 等。H.264 全平台可播；macOS 原生支持 HEVC/H.265 与 VP9。 |

Linux 上建议安装 `gstreamer1.0-libav` 和 `gstreamer1.0-plugins-good` 以获得更好的视频播放支持。

## 从源码构建

```bash
# 依赖：Rust、Node.js、pnpm、CMake、NASM
# Linux 另需：
# sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev \
#   patchelf nasm clang pkg-config autoconf automake libtool cmake

pnpm --dir src-vite install
pnpm --dir src-vite exec tauri build          # 安装包
pnpm --dir src-vite exec tauri build --no-bundle   # 仅可执行文件（便携版）
```

便携版由可执行文件加上 `src-tauri/resources` 下的 `ffmpeg` 与 `models` 两个目录组成，
三者放在同一目录即可运行。

## 技术架构

- 核心：Tauri + Rust
- 界面：Vue + Vite + Tailwind CSS
- 数据：SQLite

| 组件 | 用途 |
| :-- | :-- |
| [LibRaw](https://github.com/LibRaw/LibRaw) | RAW 解码与嵌入预览提取 |
| [libheif](https://github.com/strukturag/libheif) | HEIC/HEIF/HIF 解码 |
| [libjpeg-turbo](https://libjpeg-turbo.org/) | 快速 JPEG 解码与缩略图生成 |
| [FFmpeg](https://ffmpeg.org/) | 视频处理与缩略图生成 |
| [Video.js](https://videojs.com/) | 跨平台视频播放 |
| [ONNX Runtime](https://onnxruntime.ai/) | 本地 AI 推理引擎 |
| [CLIP](https://github.com/openai/CLIP) | 图文相似度搜索 |
| [InsightFace](https://github.com/deepinsight/insightface) | 人脸检测与识别 |
| [Leaflet](https://leafletjs.com/) | 地理位置地图 |
| [daisyUI](https://daisyui.com/) | 界面组件库 |

## 许可证

GPL-3.0-or-later，见 [LICENSE](LICENSE)。

本项目是 [Lap](https://github.com/julyx10/lap) 的衍生作品，依据 GPL-3.0 以相同许可证发布，
并保留原作者版权声明。上游各语言的项目说明保留在 [i18n/](i18n/) 目录。

---

<a name="english"></a>

## English

**猫叔的图 (Maoshu Photos)** is an offline-first local photo manager for large
personal and family libraries. Nothing is uploaded, no account is required, and
indexing, search and face recognition all run on your own machine.

> **This project is a fork of [Lap](https://github.com/julyx10/lap)** by julyx10,
> licensed GPL-3.0-or-later. The upstream engine — indexing, thumbnails, RAW
> decoding, CLIP semantic search, face clustering — is retained; the interface
> and the workflow layer built on top of it are this project's own work.
> See [NOTICE](NOTICE) for full attribution.

Licensed under GPL-3.0-or-later. As a derivative work it is distributed under
the same license, with the original copyright notices preserved. Complete
source code is provided with every binary release.
