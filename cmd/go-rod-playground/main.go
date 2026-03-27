package main

import (
	"context"
	"flag"
	"fmt"
	"math/rand"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"

	"github.com/go-rod/rod"
	"github.com/go-rod/rod/lib/proto"
	"github.com/sirupsen/logrus"
	"github.com/xpzouying/xiaohongshu-mcp/browser"
	"github.com/xpzouying/xiaohongshu-mcp/xiaohongshu"
)

type matchedPost struct {
	section *rod.Element
	title   string
	href    string
}

func main() {
	var binPath string
	var keywordsStr string
	var excludeStr string
	flag.StringVar(&binPath, "bin", "", "浏览器二进制文件路径")
	flag.StringVar(&keywordsStr, "keywords", "", "搜索关键词（英文逗号分隔）")
	flag.StringVar(&excludeStr, "exclude", "", "排除关键词（英文逗号分隔）")
	flag.Parse()

	rand.Seed(time.Now().UnixNano())

	keywords := parseKeywords(keywordsStr)
	excludeKeywords := parseKeywords(excludeStr)

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

	scanAndScroll(page, keywords, excludeKeywords)
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

func scanAndScroll(page *rod.Page, keywords, excludeKeywords []string) {
	seen_href := make(map[string]bool)
	seen_title := make(map[string]bool)

	for {
		var matched []matchedPost

		sections := page.MustElements(`#exploreFeeds section`)
		for _, section := range sections {
			href := extractHref(section)
			if href == "" || seen_href[href] {
				continue
			}
			seen_href[href] = true

			title := extractTitle(section)
			if title == "" || seen_title[title] {
				continue
			}
			seen_title[title] = true

			if containsAnyKeyword(title, excludeKeywords) {
				continue
			}

			if len(keywords) > 0 && containsAnyKeyword(title, keywords) {
				logrus.Infof("找到: %s | %s", title, href)
				matched = append(matched, matchedPost{
					section: section,
					title:   title,
					href:    href,
				})
			}
		}

		for i, post := range matched {
			logrus.Infof("浏览 [%d/%d]: %s", i+1, len(matched), post.title)
			browsePost(page, post.section)
			if i < len(matched)-1 {
				time.Sleep(randomDuration(1000, 2000))
			}
		}

		viewportHeight := page.MustEval(`() => window.innerHeight`).Int()
		scrollAmount := float64(viewportHeight) * 0.7
		page.MustEval(`(amount) => window.scrollBy({top: amount, behavior: 'smooth'})`, scrollAmount)
		time.Sleep(randomDuration(2000, 5000))
	}
}

func browsePost(page *rod.Page, section *rod.Element) {
	if err := section.Click(proto.InputMouseButtonLeft, 1); err != nil {
		logrus.Warnf("点击帖子失败: %v", err)
		return
	}

	time.Sleep(randomDuration(500, 1000))

	if err := scrollCommentsToBottom(page); err != nil {
		logrus.Warnf("滚动评论失败: %v", err)
	}

	time.Sleep(randomDuration(300, 800))

	page.MustElement(`body > div.note-detail-mask > div.close-circle`).MustClick()
	time.Sleep(randomDuration(1000, 2000))
}

func scrollCommentsToBottom(page *rod.Page) error {
	noteContainer, err := page.Element(`#noteContainer`)
	if err != nil {
		return fmt.Errorf("找不到 noteContainer: %w", err)
	}

	scroller, err := noteContainer.Element(`div.interaction-container > div.note-scroller`)
	if err != nil {
		scroller, err = noteContainer.Element(`div.interaction-container`)
		if err != nil {
			return fmt.Errorf("找不到评论容器: %w", err)
		}
	}

	maxIterations := 5 + rand.Intn(6)
	prevScrollTop := -1

	for i := 0; i < maxIterations; i++ {
		clientHeight := scroller.MustEval(`() => this.clientHeight`).Int()
		scrollTop := scroller.MustEval(`() => this.scrollTop`).Int()

		if scrollTop == prevScrollTop && i > 0 {
			logrus.Debug("已滚动到评论底部")
			break
		}
		prevScrollTop = scrollTop

		scrollAmount := float64(clientHeight) * 0.7
		scroller.MustEval(`(amount) => this.scrollBy({top: amount, behavior: 'smooth'})`, scrollAmount)
		time.Sleep(randomDuration(1500, 3000))
	}

	return nil
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
		if strings.Contains(strings.ToLower(title), strings.ToLower(kw)) {
			return true
		}
	}
	return false
}

func randomDuration(min, max int) time.Duration {
	ms := min + rand.Intn(max-min)
	return time.Duration(ms) * time.Millisecond
}
