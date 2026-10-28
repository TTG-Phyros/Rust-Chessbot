# ProgressOneOf

See [`TrainingProgress`].

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
**kind** | **str** |  | 

## Example

```python
from chess_nnue_lab.models.progress_one_of import ProgressOneOf

# TODO update the JSON string below
json = "{}"
# create an instance of ProgressOneOf from a JSON string
progress_one_of_instance = ProgressOneOf.from_json(json)
# print the JSON string representation of the object
print(ProgressOneOf.to_json())

# convert the object into a dict
progress_one_of_dict = progress_one_of_instance.to_dict()
# create an instance of ProgressOneOf from a dict
progress_one_of_from_dict = ProgressOneOf.from_dict(progress_one_of_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


