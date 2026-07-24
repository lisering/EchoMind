# Malicious Markdown Test

## XSS 攻击向量

<script>alert('XSS')</script>

<img src=x onerror="alert(document.cookie)">

<a href="javascript:void(0)">Click me</a>

<iframe src="javascript:alert(1)"></iframe>

## 正常内容

这段文字应该被保留。MarkdownLoader 应剥离上面的 HTML 标签但保留正文。

## 代码块中的 script（安全，应保留）

```html
<script>alert('safe in code block')</script>
```

## 混合内容

正常段落。<script>alert('inline')</script> 另一段正常文字。

<img src="valid.png" onerror="alert(1)" alt="image">

## 结尾正常文字

这是文件末尾的正常文字，用于验证恶意标签不会截断正文提取。
