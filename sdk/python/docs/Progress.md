# Progress

Latest progress of a job.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**learning_rate** | **float** | Learning rate of the last superbatch. | 
**loss** | **float** | Mean loss of the last superbatch. | 
**positions** | **int** | Positions recorded. | 
**positions_per_second** | **float** | Speed of the last superbatch. | 
**step** | **int** | Optimizer steps done. | 
**superbatch** | **int** | Superbatches done. | 
**superbatches** | **int** | Superbatches to do. | 
**vram_used_mb** | **int** | GPU memory in use, when on a GPU. | [optional] 
**kind** | **str** |  | 
**games** | **int** | Games played. | 
**games_total** | **int** | Games to play. | 
**baseline_nps** | **int** | Nodes per second of the baseline&#39;s search, on the same worker. | [optional] 
**candidate_nps** | **int** | Nodes per second of the candidate&#39;s search (one thread, measured by the worker before the match). | [optional] 
**draws** | **int** | Games drawn. | 
**elo** | **float** | Elo difference of the candidate over the baseline. | 
**elo_error** | **float** | Half-width of the 95 % confidence interval of &#x60;elo&#x60;. | 
**games_max** | **int** | Games to play at most. | 
**los** | **float** | Likelihood of superiority: probability that the candidate is stronger. | 
**losses** | **int** | Games the candidate lost. | 
**score** | **float** | Candidate&#39;s mean score, in [0, 1]. | 
**sprt** | [**SprtStatus**](SprtStatus.md) | The SPRT, when the match runs one. | [optional] 
**wins** | **int** | Games the candidate won. | 

## Example

```python
from chess_nnue_lab.models.progress import Progress

# TODO update the JSON string below
json = "{}"
# create an instance of Progress from a JSON string
progress_instance = Progress.from_json(json)
# print the JSON string representation of the object
print(Progress.to_json())

# convert the object into a dict
progress_dict = progress_instance.to_dict()
# create an instance of Progress from a dict
progress_from_dict = Progress.from_dict(progress_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


