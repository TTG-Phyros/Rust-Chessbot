// Trains a network through the NNUE lab API with net/http only: validates
// and saves examples/architecture.json, uploads a dataset, trains, follows
// the live events, downloads the model, evaluates it against the classical
// evaluation, then activates it.
//
//	cd examples/go
//	CHESS_API_URL=http://localhost:8080 CHESS_API_KEY=... go run . positions.cbds
package main

import (
	"bufio"
	"bytes"
	"crypto/rand"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
)

// Client calls the API with a key.
type Client struct {
	base string
	key  string
}

// Problem is an error response (RFC 9457).
type Problem struct {
	Type   string `json:"type"`
	Title  string `json:"title"`
	Status int    `json:"status"`
	Detail string `json:"detail"`
	Errors []struct {
		Path    string `json:"path"`
		Message string `json:"message"`
	} `json:"errors"`
}

func (p *Problem) Error() string {
	text := fmt.Sprintf("HTTP %d %s: %s", p.Status, p.Title, p.Detail)
	for _, e := range p.Errors {
		text += fmt.Sprintf("\n  %s: %s", e.Path, e.Message)
	}
	return text
}

// do sends a request and returns the response, or the problem it carries.
func (c *Client) do(method, path, contentType string, body io.Reader, headers map[string]string) (*http.Response, error) {
	req, err := http.NewRequest(method, c.base+"/api/v1"+path, body)
	if err != nil {
		return nil, err
	}
	req.Header.Set("X-API-Key", c.key)
	if contentType != "" {
		req.Header.Set("Content-Type", contentType)
	}
	for name, value := range headers {
		req.Header.Set(name, value)
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, err
	}
	if resp.StatusCode >= 400 {
		defer resp.Body.Close()
		problem := &Problem{Status: resp.StatusCode}
		if err := json.NewDecoder(resp.Body).Decode(problem); err != nil {
			problem.Title = resp.Status
		}
		return nil, problem
	}
	return resp, nil
}

// json sends a request and decodes the JSON answer into out (if not nil).
func (c *Client) json(method, path string, in, out any, headers map[string]string) error {
	var body io.Reader
	contentType := ""
	if in != nil {
		data, err := json.Marshal(in)
		if err != nil {
			return err
		}
		body, contentType = bytes.NewReader(data), "application/json"
	}
	resp, err := c.do(method, path, contentType, body, headers)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if out == nil {
		return nil
	}
	return json.NewDecoder(resp.Body).Decode(out)
}

// follow prints the live events (Server-Sent Events) of a job until it ends.
func (c *Client) follow(path string) error {
	resp, err := c.do("GET", path, "", nil, nil)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	lines := bufio.NewScanner(resp.Body)
	for lines.Scan() {
		data, ok := strings.CutPrefix(lines.Text(), "data:")
		if !ok {
			continue
		}
		var event JobEvent
		if err := json.Unmarshal([]byte(data), &event); err != nil {
			return err
		}
		d := event.Data
		switch {
		case event.Kind == "status":
			fmt.Printf("  status: %s\n", d.Status)
		case event.Kind == "progress" && d.Kind == "training":
			fmt.Printf("  superbatch %d/%d: loss %v\n", d.Superbatch, d.Superbatches, d.Loss)
		case event.Kind == "progress" && d.Kind == "evaluation" && d.Games > 0:
			fmt.Printf("  games %d/%d: +%d =%d -%d\n", d.Games, d.GamesMax, d.Wins, d.Draws, d.Losses)
		}
	}
	return lines.Err()
}

func newKey() string {
	b := make([]byte, 16)
	rand.Read(b)
	return fmt.Sprintf("%x", b)
}

type Estimate struct {
	Parameters   int64 `json:"parameters"`
	WeightsBytes int64 `json:"weightsBytes"`
}

type Dataset struct {
	ID      string `json:"id"`
	Name    string `json:"name"`
	Records int64  `json:"records"`
	Sha256  string `json:"sha256"`
}

type Job struct {
	ID     string `json:"id"`
	Status string `json:"status"`
	Error  string `json:"error"`
	Model  string `json:"model"`
}

type JobEvent struct {
	Kind string `json:"kind"`
	Data struct {
		Status string `json:"status"`
		// Progress: of a training run, or of an evaluation.
		Kind         string  `json:"kind"`
		Superbatch   int64   `json:"superbatch"`
		Superbatches int64   `json:"superbatches"`
		Loss         float64 `json:"loss"`
		Games        int64   `json:"games"`
		GamesMax     int64   `json:"gamesMax"`
		Wins         int64   `json:"wins"`
		Draws        int64   `json:"draws"`
		Losses       int64   `json:"losses"`
	} `json:"data"`
}

// Evaluation is an evaluation job: its contestants and its latest result.
type Evaluation struct {
	ID     string `json:"id"`
	Status string `json:"status"`
	Error  string `json:"error"`
	Spec   struct {
		Candidate struct {
			Parameters int64 `json:"parameters"`
			Bytes      int64 `json:"bytes"`
		} `json:"candidate"`
	} `json:"spec"`
	Progress struct {
		Wins     int64   `json:"wins"`
		Draws    int64   `json:"draws"`
		Losses   int64   `json:"losses"`
		Elo      float64 `json:"elo"`
		EloError float64 `json:"eloError"`
		Los      float64 `json:"los"`
		Sprt     struct {
			Verdict string `json:"verdict"`
		} `json:"sprt"`
		CandidateNps int64 `json:"candidateNps"`
		BaselineNps  int64 `json:"baselineNps"`
	} `json:"progress"`
}

func run(c *Client, datasetPath string) error {
	// 1. Validate the architecture: errors, size and cost before training anything.
	architecture, err := os.ReadFile(filepath.Join("..", "architecture.json"))
	if err != nil {
		return err
	}
	var report struct {
		Valid    bool     `json:"valid"`
		Hash     string   `json:"hash"`
		Estimate Estimate `json:"estimate"`
		Errors   []struct {
			Path    string `json:"path"`
			Message string `json:"message"`
		} `json:"errors"`
	}
	if err := c.json("POST", "/architectures:validate", json.RawMessage(architecture), &report, nil); err != nil {
		return err
	}
	if !report.Valid {
		for _, e := range report.Errors {
			fmt.Fprintf(os.Stderr, "%s: %s\n", e.Path, e.Message)
		}
		return fmt.Errorf("invalid architecture")
	}
	fmt.Printf("architecture %s: %d parameters, %d bytes\n",
		report.Hash[:12], report.Estimate.Parameters, report.Estimate.WeightsBytes)

	// 2. Save it. An Idempotency-Key makes the request safe to retry.
	var saved struct {
		ID string `json:"id"`
	}
	if err := c.json("POST", "/architectures", json.RawMessage(architecture), &saved,
		map[string]string{"Idempotency-Key": newKey()}); err != nil {
		return err
	}

	// 3. Upload the dataset, streamed from disk (the server checks every record).
	file, err := os.Open(datasetPath)
	if err != nil {
		return err
	}
	defer file.Close()
	name := filepath.Base(datasetPath)
	resp, err := c.do("POST", "/datasets?name="+url.QueryEscape(name), "application/octet-stream", file, nil)
	if err != nil {
		return err
	}
	var dataset Dataset
	err = json.NewDecoder(resp.Body).Decode(&dataset)
	resp.Body.Close()
	if err != nil {
		return err
	}
	fmt.Printf("dataset %s: %d positions, sha256 %s\n", dataset.Name, dataset.Records, dataset.Sha256[:12])

	// 4. Queue a short training run on the saved architecture.
	request := map[string]any{
		"name":           "go example",
		"architectureId": saved.ID,
		"datasets":       []string{dataset.ID},
		"seed":           1,
		"hyperparameters": map[string]any{
			"superbatches": 3, "batchesPerSuperbatch": 50, "batchSize": 1024,
		},
	}
	var job Job
	if err := c.json("POST", "/training-runs", request, &job, nil); err != nil {
		return err
	}
	fmt.Printf("training run %s queued\n", job.ID)

	// 5. Follow its live events (Server-Sent Events) until the run is over.
	if err := c.follow("/training-runs/" + job.ID + "/events"); err != nil {
		return err
	}
	if err := c.json("GET", "/training-runs/"+job.ID, nil, &job, nil); err != nil {
		return err
	}
	if job.Status != "succeeded" {
		return fmt.Errorf("run ended %s: %s", job.Status, job.Error)
	}
	model := job.Model

	// 6. Download the network file.
	if err := download(c, model); err != nil {
		return err
	}
	fmt.Printf("model %s saved to %s.nnue\n", model, model)

	// 7. Evaluate it: a match against the classical evaluation, at a fixed
	//    number of nodes per move, stopped as soon as the SPRT concludes.
	evaluationRequest := map[string]any{
		"candidate": model,
		"games":     20,
		"nodes":     1000,
		"sprt":      map[string]any{"elo0": 0, "elo1": 50},
		"seed":      1,
		"threads":   2,
	}
	var evaluation Evaluation
	if err := c.json("POST", "/evaluations", evaluationRequest, &evaluation, nil); err != nil {
		return err
	}
	fmt.Printf("evaluation %s queued\n", evaluation.ID)
	if err := c.follow("/evaluations/" + evaluation.ID + "/events"); err != nil {
		return err
	}
	if err := c.json("GET", "/evaluations/"+evaluation.ID, nil, &evaluation, nil); err != nil {
		return err
	}
	if evaluation.Status != "succeeded" {
		return fmt.Errorf("evaluation ended %s: %s", evaluation.Status, evaluation.Error)
	}
	r := evaluation.Progress
	fmt.Printf("against the classical evaluation: +%d =%d -%d, Elo %+.1f ± %.1f, LOS %.1f%%, SPRT %s\n",
		r.Wins, r.Draws, r.Losses, r.Elo, r.EloError, 100*r.Los, r.Sprt.Verdict)
	fmt.Printf("speed %d nodes/s (classical: %d), network %d parameters, %d bytes\n",
		r.CandidateNps, r.BaselineNps, evaluation.Spec.Candidate.Parameters, evaluation.Spec.Candidate.Bytes)

	// 8. Make it the engine's network.
	if err := c.json("POST", "/models/"+model+":activate", nil, nil, nil); err != nil {
		return err
	}
	fmt.Printf("model %s active\n", model)
	return nil
}

// download saves the network file of a model as <model>.nnue.
func download(c *Client, model string) error {
	resp, err := c.do("GET", "/models/"+model+"/file", "", nil, nil)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	out, err := os.Create(model + ".nnue")
	if err != nil {
		return err
	}
	defer out.Close()
	_, err = io.Copy(out, resp.Body)
	return err
}

func main() {
	if len(os.Args) != 2 {
		fmt.Fprintln(os.Stderr, "usage: go run . DATASET.cbds")
		os.Exit(1)
	}
	base := os.Getenv("CHESS_API_URL")
	if base == "" {
		base = "http://localhost:8080"
	}
	client := &Client{base: strings.TrimSuffix(base, "/"), key: os.Getenv("CHESS_API_KEY")}
	if err := run(client, os.Args[1]); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
