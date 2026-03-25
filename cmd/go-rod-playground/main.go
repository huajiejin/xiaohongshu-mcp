package main

import (
	"context"
	"flag"
	"math/rand"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"

	"github.com/go-rod/rod"
	"github.com/sirupsen/logrus"
	"github.com/xpzouying/xiaohongshu-mcp/browser"
	"github.com/xpzouying/xiaohongshu-mcp/xiaohongshu"
)

func main() {
	var binPath string
	var keywordsStr string
	flag.StringVar(&binPath, "bin", "", "浏览器二进制文件路径")
	flag.StringVar(&keywordsStr, "keywords", "", "搜索关键词（英文逗号分隔）")
	flag.Parse()

	rand.Seed(time.Now().UnixNano())

	keywords := parseKeywords(keywordsStr)

	b := browser.NewBrowser(false, browser.WithBinPath(binPath))
	defer b.Close()

	page := b.NewPage()
	page.MustWindowMaximize()
	defer page.Close()

	logrus.Info("正在打开小红书...")
	page.MustNavigate("https://www.xiaohongshu.com/explore").MustWaitLoad()

	login := xiaohongshu.NewLogin(page)
	loggedIn, err := login.CheckLoginStatus(context.Background())
	if err != nil {
		logrus.Warnf("检查登录状态失败: %v", err)
	} else if loggedIn {
		logrus.Info("已登录")
	} else {
		logrus.Warn("未登录，请先运行 login 命令")
	}

	if len(keywords) > 0 {
		logrus.Infof("开始扫描关键词: %s (按 Ctrl+C 退出)", keywordsStr)
	} else {
		logrus.Info("开始浏览 (按 Ctrl+C 退出)")
	}

	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, os.Interrupt, syscall.SIGTERM)

	go func() {
		<-sigChan
		logrus.Info("正在退出...")
		os.Exit(0)
	}()

	scanAndScroll(page, keywords)
}

func parseKeywords(s string) []string {
	if s == "" {
		return nil
	}
	parts := strings.Split(s, ",")
	result := make([]string, 0, len(parts))
	for _, p := range parts {
		trimmed := strings.TrimSpace(p)
		if trimmed != "" {
			result = append(result, trimmed)
		}
	}
	return result
}

func scanAndScroll(page *rod.Page, keywords []string) {
	seen := make(map[string]bool)

	for {
		sections := page.MustElements(`#exploreFeeds section`)

		for _, section := range sections {
			href := extractHref(section)
			if href == "" || seen[href] {
				continue
			}
			seen[href] = true

			title := extractTitle(section)
			if title == "" {
				continue
			}

			if len(keywords) > 0 && containsAnyKeyword(title, keywords) {
				logrus.Infof("找到: %s | %s", title, href)
			}
		}

		viewportHeight := page.MustEval(`() => window.innerHeight`).Int()
		scrollAmount := float64(viewportHeight) * 0.7
		page.MustEval(`(amount) => window.scrollBy(0, amount)`, scrollAmount)

		delay := randomDuration(2000, 5000)
		time.Sleep(delay)
	}
}

func extractHref(section *rod.Element) string {
	a, err := section.Element(`a.cover`)
	if err != nil {
		return ""
	}
	href, err := a.Property("href")
	if err != nil {
		return ""
	}
	return href.String()
}

func extractTitle(section *rod.Element) string {
	span, err := section.Element(`div > div > a > span`)
	if err != nil {
		return ""
	}
	title, err := span.Text()
	if err != nil {
		return ""
	}
	return title
}

func containsAnyKeyword(title string, keywords []string) bool {
	for _, kw := range keywords {
		if strings.Contains(title, kw) {
			return true
		}
	}
	return false
}

func randomDuration(min, max int) time.Duration {
	ms := min + rand.Intn(max-min)
	return time.Duration(ms) * time.Millisecond
}
