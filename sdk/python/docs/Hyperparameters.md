# Hyperparameters

How a network is trained. Every field has a default.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**batch_size** | **int** | Positions per optimizer step. | [optional] [default to 16384]
**batches_per_superbatch** | **int** | Steps per superbatch. | [optional] [default to 200]
**learning_rate** | **float** | AdamW learning rate at the start. | [optional] [default to 0.001]
**lr_drop_every** | **int** | The learning rate is multiplied by &#x60;lrDropFactor&#x60; every &#x60;lrDropEvery&#x60; superbatches. | [optional] [default to 15]
**lr_drop_factor** | **float** | See &#x60;lrDropEvery&#x60;. | [optional] [default to 0.3]
**score_scale** | **float** | Centipawns per unit of the sigmoid of the target and the loss. | [optional] [default to 400.0]
**shuffle_records** | **int** | Positions shuffled together by the loader (32 bytes of memory each). | [optional] [default to 2000000]
**superbatches** | **int** | Superbatches to train (a checkpoint is saved after each). | [optional] [default to 40]
**wdl** | **float** | Weight of the game result in the target; the rest is the search score: &#x60;(1 − wdl)·σ(score / scoreScale) + wdl·result&#x60;. | [optional] [default to 0.25]

## Example

```python
from chess_nnue_lab.models.hyperparameters import Hyperparameters

# TODO update the JSON string below
json = "{}"
# create an instance of Hyperparameters from a JSON string
hyperparameters_instance = Hyperparameters.from_json(json)
# print the JSON string representation of the object
print(Hyperparameters.to_json())

# convert the object into a dict
hyperparameters_dict = hyperparameters_instance.to_dict()
# create an instance of Hyperparameters from a dict
hyperparameters_from_dict = Hyperparameters.from_dict(hyperparameters_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


