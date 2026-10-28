# SelfplayStarted

A self-play run and the dataset it fills.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**dataset** | [**Dataset**](Dataset.md) | The dataset, &#x60;generating&#x60; until the run ends. | 
**run** | [**Job**](Job.md) | The run (see &#x60;/api/v1/selfplay-runs/{id}&#x60;). | 

## Example

```python
from chess_nnue_lab.models.selfplay_started import SelfplayStarted

# TODO update the JSON string below
json = "{}"
# create an instance of SelfplayStarted from a JSON string
selfplay_started_instance = SelfplayStarted.from_json(json)
# print the JSON string representation of the object
print(SelfplayStarted.to_json())

# convert the object into a dict
selfplay_started_dict = selfplay_started_instance.to_dict()
# create an instance of SelfplayStarted from a dict
selfplay_started_from_dict = SelfplayStarted.from_dict(selfplay_started_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


