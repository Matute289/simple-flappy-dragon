package main

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"testing"
)

func withTempCSV(t *testing.T, fn func()) {
	t.Helper()
	f, err := os.CreateTemp("", "scores*.csv")
	if err != nil {
		t.Fatal(err)
	}
	f.Close()
	t.Setenv("SCORES_CSV_PATH", f.Name())
	defer os.Remove(f.Name())
	fn()
}

func TestValidateEntry(t *testing.T) {
	cases := []struct {
		e     Entry
		valid bool
	}{
		{Entry{"Matias", 42}, true},
		{Entry{"", 42}, false},
		{Entry{"x", -1}, false},
		{Entry{"x", 10000}, false},
		{Entry{strings.Repeat("a", 33), 5}, false},
		{Entry{strings.Repeat("a", 32), 5}, true},
	}
	for _, c := range cases {
		err := validateEntry(c.e)
		if (err == nil) != c.valid {
			t.Errorf("validateEntry(%v) want valid=%v, got err=%v", c.e, c.valid, err)
		}
	}
}

func TestSortAndTrim(t *testing.T) {
	entries := []Entry{{"a", 10}, {"b", 30}, {"c", 20}}
	result := sortAndTrim(entries, 2)
	if len(result) != 2 || result[0].Score != 30 || result[1].Score != 20 {
		t.Errorf("wrong sort/trim: %v", result)
	}
}

func TestGetEmpty(t *testing.T) {
	withTempCSV(t, func() {
		req := httptest.NewRequest(http.MethodGet, "/", nil)
		w := httptest.NewRecorder()
		handler(w, req)
		if w.Code != http.StatusOK {
			t.Fatalf("expected 200, got %d", w.Code)
		}
		var entries []Entry
		json.NewDecoder(w.Body).Decode(&entries)
		if len(entries) != 0 {
			t.Errorf("expected empty slice, got %v", entries)
		}
	})
}

func TestPostAndGet(t *testing.T) {
	withTempCSV(t, func() {
		body := bytes.NewBufferString(`{"name":"Matias","score":42}`)
		req := httptest.NewRequest(http.MethodPost, "/", body)
		req.Header.Set("Content-Type", "application/json")
		w := httptest.NewRecorder()
		handler(w, req)
		if w.Code != http.StatusOK {
			t.Fatalf("POST expected 200, got %d: %s", w.Code, w.Body.String())
		}

		req2 := httptest.NewRequest(http.MethodGet, "/", nil)
		w2 := httptest.NewRecorder()
		handler(w2, req2)
		var entries []Entry
		json.NewDecoder(w2.Body).Decode(&entries)
		if len(entries) != 1 || entries[0].Name != "Matias" || entries[0].Score != 42 {
			t.Errorf("unexpected entries: %v", entries)
		}
	})
}

func TestPostInvalidScore(t *testing.T) {
	withTempCSV(t, func() {
		body := bytes.NewBufferString(`{"name":"x","score":99999}`)
		req := httptest.NewRequest(http.MethodPost, "/", body)
		req.Header.Set("Content-Type", "application/json")
		w := httptest.NewRecorder()
		handler(w, req)
		if w.Code != http.StatusBadRequest {
			t.Fatalf("expected 400, got %d", w.Code)
		}
	})
}

func TestPostEmptyName(t *testing.T) {
	withTempCSV(t, func() {
		body := bytes.NewBufferString(`{"name":"","score":10}`)
		req := httptest.NewRequest(http.MethodPost, "/", body)
		req.Header.Set("Content-Type", "application/json")
		w := httptest.NewRecorder()
		handler(w, req)
		if w.Code != http.StatusBadRequest {
			t.Fatalf("expected 400, got %d", w.Code)
		}
	})
}
