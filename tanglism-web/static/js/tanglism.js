import { ajax_params, validate_ajax_params } from './tanglism-common.js';
import { kline } from './tanglism-kline.js';
import { stroke } from './tanglism-stroke.js';
import { segment } from './tanglism-segment.js';
import { subtrend } from './tanglism-subtrend.js';
import { center } from './tanglism-center.js';

$(document).ready(function() {
  // 将各形态函数注册到K线回调上
  kline.add_draw_callback(stroke.draw);
  kline.add_draw_callback(segment.draw);
  kline.add_draw_callback(subtrend.draw);
  kline.add_draw_callback(center.draw);
  // 将各形态数据过期关联到K线数据回调
  kline.add_data_callback(stroke.outdate);
  kline.add_data_callback(segment.outdate);
  kline.add_data_callback(subtrend.outdate);
  kline.add_data_callback(center.outdate);
  // 股票选择
  $("#input_stock_code").autocomplete({
    source: function(req, callback) {
      $.ajax({
        url: "api/v1/keyword-stocks?keyword=" + encodeURIComponent(req.term),
        method: "GET",
        dataType: "json",
        success: function(resp) {
          callback($.map(resp.content, function(item){
            return {
              value: item.code,
              label: item.code + " " + item.display_name
            };
          }));
        },
        error: function(err) {
          console.log("ajax error on search stock", err);
          callback([]);
        }
      })
    }
  });
  // 时间选择
  $("#input_start_dt").datepicker({
    dateFormat: "yy-mm-dd",
    minDate: "2010-01-01",
    maxDate: -1
  });
  $("#input_end_dt").datepicker({
    dateFormat: "yy-mm-dd",
    minDate: "2010-01-01",
    maxDate: -1
  });
  $("#data_container").tabs();
  // 笔逻辑选择
  $("input[name='stroke_logic_gap']").click(function(e){
    var value = $(this).val();
    if (value === "gap_ratio") {
      $("#gap_ratio_percentage_span").css("display", "inline");
    } else {
      $("#gap_ratio_percentage_span").css("display", "none");
    }
  });
  // 提交事件
  $(".stock_submission_trigger").change(function() {
    if (validate_ajax_params()) {
      kline.ajax(ajax_params());
    }
  });
  // 柱间距选择
  $("input[name='bar_padding']").click(function(e){
    var value = $(this).val();
    if (value === "fixed") {
      $("#bar_padding_fixed_width_span").css("display", "inline");
    } else {
      $("#bar_padding_fixed_width_span").css("display", "none");
    }
  });
  // 画图事件
  $(".draw_trigger").change(function() {
    if (validate_ajax_params()) {
      kline.draw();
    }
  })
});

