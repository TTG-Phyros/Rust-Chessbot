# MetricPoint

One point of a training curve.

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
**time** | **int** | When it was reported (Unix milliseconds). | 

## Example

```python
from chess_nnue_lab.models.metric_point import MetricPoint

# TODO update the JSON string below
json = "{}"
# create an instance of MetricPoint from a JSON string
metric_point_instance = MetricPoint.from_json(json)
# print the JSON string representation of the object
print(MetricPoint.to_json())

# convert the object into a dict
metric_point_dict = metric_point_instance.to_dict()
# create an instance of MetricPoint from a dict
metric_point_from_dict = MetricPoint.from_dict(metric_point_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


