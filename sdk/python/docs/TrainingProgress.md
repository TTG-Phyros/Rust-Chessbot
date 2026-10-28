# TrainingProgress

Latest progress of a training job.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**learning_rate** | **float** | Learning rate of the last superbatch. | 
**loss** | **float** | Mean loss of the last superbatch. | 
**positions** | **int** | Positions trained on. | 
**positions_per_second** | **float** | Speed of the last superbatch. | 
**step** | **int** | Optimizer steps done. | 
**superbatch** | **int** | Superbatches done. | 
**superbatches** | **int** | Superbatches to do. | 
**vram_used_mb** | **int** | GPU memory in use, when on a GPU. | [optional] 

## Example

```python
from chess_nnue_lab.models.training_progress import TrainingProgress

# TODO update the JSON string below
json = "{}"
# create an instance of TrainingProgress from a JSON string
training_progress_instance = TrainingProgress.from_json(json)
# print the JSON string representation of the object
print(TrainingProgress.to_json())

# convert the object into a dict
training_progress_dict = training_progress_instance.to_dict()
# create an instance of TrainingProgress from a dict
training_progress_from_dict = TrainingProgress.from_dict(training_progress_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


