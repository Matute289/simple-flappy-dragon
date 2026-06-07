package main

import (
	"encoding/csv"
	"encoding/json"
	"errors"
	"net/http"
	"os"
	"sort"
	"strconv"
	"strings"
	"sync"
)

const (
	maxEntries = 1000
	maxNameLen = 32
	maxScore   = 9999
)

type Entry struct {
	Name  string `json:"name"`
	Score int    `json:"score"`
}

var mu sync.RWMutex

func csvPath() string {
	if p := os.Getenv("SCORES_CSV_PATH"); p != "" {
		return p
	}
	return "/data/scores.csv"
}

func readCSV() ([]Entry, error) {
	f, err := os.Open(csvPath())
	if errors.Is(err, os.ErrNotExist) {
		return []Entry{}, nil
	}
	if err != nil {
		return nil, err
	}
	defer f.Close()

	records, err := csv.NewReader(f).ReadAll()
	if err != nil {
		return nil, err
	}

	var entries []Entry
	for i, rec := range records {
		if i == 0 && len(rec) > 0 && rec[0] == "name" {
			continue
		}
		if len(rec) < 2 {
			continue
		}
		score, err := strconv.Atoi(rec[1])
		if err != nil {
			continue
		}
		entries = append(entries, Entry{Name: rec[0], Score: score})
	}
	if entries == nil {
		return []Entry{}, nil
	}
	return entries, nil
}

func writeCSV(entries []Entry) error {
	entries = sortAndTrim(entries, maxEntries)
	f, err := os.Create(csvPath())
	if err != nil {
		return err
	}
	defer f.Close()
	w := csv.NewWriter(f)
	w.Write([]string{"name", "score"})
	for _, e := range entries {
		w.Write([]string{e.Name, strconv.Itoa(e.Score)})
	}
	w.Flush()
	return w.Error()
}

func sortAndTrim(entries []Entry, max int) []Entry {
	sort.Slice(entries, func(i, j int) bool {
		return entries[i].Score > entries[j].Score
	})
	if len(entries) > max {
		return entries[:max]
	}
	return entries
}

func validateEntry(e Entry) error {
	name := strings.TrimSpace(e.Name)
	if name == "" {
		return errors.New("name required")
	}
	if len(name) > maxNameLen {
		return errors.New("name too long")
	}
	if e.Score < 0 || e.Score > maxScore {
		return errors.New("score out of range")
	}
	return nil
}

func handler(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Access-Control-Allow-Origin", "*")

	switch r.Method {
	case http.MethodOptions:
		w.Header().Set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type")
		w.WriteHeader(http.StatusNoContent)

	case http.MethodGet:
		mu.RLock()
		entries, err := readCSV()
		mu.RUnlock()
		if err != nil {
			http.Error(w, "internal error", http.StatusInternalServerError)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(entries)

	case http.MethodPost:
		var entry Entry
		if err := json.NewDecoder(r.Body).Decode(&entry); err != nil {
			http.Error(w, "invalid JSON", http.StatusBadRequest)
			return
		}
		entry.Name = strings.TrimSpace(entry.Name)
		if err := validateEntry(entry); err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}
		mu.Lock()
		entries, err := readCSV()
		if err == nil {
			entries = append(entries, entry)
			err = writeCSV(entries)
		}
		mu.Unlock()
		if err != nil {
			http.Error(w, "internal error", http.StatusInternalServerError)
			return
		}
		w.WriteHeader(http.StatusOK)

	default:
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
	}
}

func main() {
	http.HandleFunc("/", handler)
	http.ListenAndServe(":8080", nil)
}
