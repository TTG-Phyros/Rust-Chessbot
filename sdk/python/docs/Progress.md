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


