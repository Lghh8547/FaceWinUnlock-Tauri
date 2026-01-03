# FaceWinUnlock-Tauri

**FaceWinUnlock-Tauri** 是一款基于 Tauri 框架开发的现代化 Windows 面容识别解锁增强软件。它通过自定义 Credential Provider (DLL) 注入 Windows 登录界面，结合前端 Vue 3 和后端 OpenCV 人脸识别算法，为用户提供类似 Windows Hello 的解锁体验。

## 通知

开发完成作者会构建发行版，建议点个star关注进度

---

## 💡 开发计划 (Roadmap)

* [x] 系统初始化向导
* [x] 实时摄像头人脸录入
* [ ] 多面容关联单账户
* [ ] 多面容关联多账户
* [ ] 识别成功后的动态磁贴反馈

---

## ✨ 特性

* **现代化 UI**: 基于 Element Plus 构建。
* **系统级集成**: 自动注册 WinLogon 凭据提供程序 (Credential Provider)。
* **双账户支持**: 同时支持本地账户 (Local Account) 与微软联机账户 (MSA) 解锁（联机账户未测试）。
* **轻量级后端**: Rust 后端确保了高效的文件 IO 处理与注册表操作安全性。
* **隐私保护**: 所有面容特征数据与系统凭据均通过 SQLite 本地存储，不上传云端。

## 🛠️ 技术栈

* **前端界面**: Vue 3 (Composition API), Vue-Router, Pinia, Element Plus
* **后端接口**: Rust (Tauri), Windows API
* **数据库**: SQLite 3
* **面容识别**: OpenCV (人脸检测与特征比对)
* **解锁组件**: 纯Rust 编写的 WinLogon 注入组件

## 📦 代码库

- [WinLogon DLL](Server/)
- [图形化界面](UI/)

## ⚠️ 免责声明

本项目涉及修改 Windows 系统注册表及 `C:\Windows\System32` 目录。在使用或二次开发时，请务必了解以下风险：

* 错误修改注册表可能导致系统无法正常登录。
* 建议在虚拟机 (VMware/Hyper-V) 环境中进行调试。
* 作者不对因使用本软件导致的任何数据丢失或系统崩溃负责。

## 📄 开源协议

本项目采用 [MIT License](LICENSE) 开源。
