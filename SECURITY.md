# Security Policy

## Supported Versions

当前支持的版本及安全更新政策：

| Version | Supported          |
| ------- | ------------------ |
| 0.2.x   | :white_check_mark: |
| 0.1.x   | :x:                |

## Reporting a Vulnerability

如果你发现了安全漏洞，请 **不要** 在公开 Issue 中报告。

### 报告方式

请通过以下方式之一私密报告：

1. **GitHub Security Advisory**（推荐）
   - 访问 [https://github.com/guangl/sqllog2db/security/advisories](https://github.com/guangl/sqllog2db/security/advisories)
   - 点击 "Report a vulnerability"
   - 填写详细信息

2. **Email**
   - 发送至：guangluo@outlook.com
   - 主题：`[SECURITY] sqllog2db vulnerability report`

### 报告内容应包括

- 漏洞描述
- 影响范围（版本、平台等）
- 复现步骤
- 潜在影响评估
- 修复建议（如果有）

### 响应时间

- **确认收到**：48 小时内
- **初步评估**：7 天内
- **修复计划**：14 天内（取决于严重程度）

### 漏洞处理流程

1. 收到报告后，我们会确认并评估严重程度
2. 制定修复计划并开发补丁
3. 发布安全公告和新版本
4. 在 CHANGELOG 和 GitHub Release Notes 中说明

### 致谢

感谢所有负责任地报告安全问题的研究人员。我们会在修复后的 Release Notes 中致谢（如果你愿意）。

## 安全最佳实践

使用 sqllog2db 时的安全建议：

### 配置文件
- 不要在配置文件中硬编码敏感信息（密码、密钥等）
- 使用环境变量或密钥管理服务
- 限制配置文件的访问权限（如 `chmod 600 config.toml`）

### 数据库连接
- 使用最小权限原则配置数据库用户
- 启用 SSL/TLS 连接（PostgreSQL 等）
- 避免使用默认凭据

### 文件权限
- 导出文件可能包含敏感数据，设置适当的文件权限
- 错误日志可能包含部分敏感信息，妥善保管

### 容器化部署
- 不要使用 root 用户运行容器
- 挂载卷时设置只读权限（如果可能）
- 定期更新基础镜像

### 依赖管理
- 定期运行 `cargo audit` 检查依赖漏洞
- 及时更新到最新稳定版本

## 已知限制

- SQL 日志可能包含敏感查询参数，导出时需注意数据安全
- 错误日志以纯文本存储，可能泄露部分原始数据
- 默认配置未加密导出文件

## 更新通知

关注以下渠道获取安全更新通知：

- GitHub Releases: [https://github.com/guangl/sqllog2db/releases](https://github.com/guangl/sqllog2db/releases)
- GitHub Security Advisories: [https://github.com/guangl/sqllog2db/security/advisories](https://github.com/guangl/sqllog2db/security/advisories)
